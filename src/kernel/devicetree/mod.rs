use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr::NonNull;

use crate::kernel::sync::spinlock::SpinLock;

use fdt::{Fdt, fdt_structure_block::StructureBlockEntryKind};
use node::Node;
use crate::kernel::devicetree::prop::Property;

pub mod fdt;
pub mod node;
pub mod prop;
pub mod std_prop;
pub mod interrupts;

static FDT: SpinLock<Option<Fdt>> = SpinLock::new(None);

pub fn set_fdt(fdt: Fdt) {
    FDT.lock().replace(fdt);
}

pub fn get_devicetree() -> DeviceTree {
    let fdt = FDT.lock();
    match fdt.as_ref() {
        None => panic!("FDT not set"),
        Some(fdt) => fdt.try_into().expect("Cannot convert FDT to DeviceTree"),
    }
}

pub struct DeviceTree {
    address: usize,
    size: u32,
    version: u32,
    last_comp_version: u32,
    boot_cpuid_phys: u32,
    memory_reservations: Vec<(usize, usize)>,
    root: Box<Node>,
}

impl DeviceTree {
    pub fn addr(&self) -> usize {
        self.address
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn last_compatible_version(&self) -> u32 {
        self.last_comp_version
    }

    pub fn boot_cpuid_phys(&self) -> u32 {
        self.boot_cpuid_phys
    }

    pub fn memory_reservations(&self) -> &Vec<(usize, usize)> {
        &self.memory_reservations
    }

    pub fn root(&self) -> &Node {
        &self.root
    }
}

impl TryFrom<&Fdt> for DeviceTree {
    type Error = ();
    fn try_from(fdt: &Fdt) -> Result<Self, Self::Error> {
        let address = fdt.address();
        let size = fdt.size();
        let version = fdt.version();
        let last_comp_version = fdt.last_compatible_version();
        let boot_cpuid_phys = fdt.boot_cpuid_phys();
        let memory_reservations: Vec<_> = fdt
            .memory_reservation_block_iter()
            .map(|r| (r.address() as usize, r.size() as usize))
            .collect();

        // TODO: refactor the concept to a builder pattern to avoid
        // node.properties_mut() and node.children_mut()
        let mut nodes_stack: Vec<Box<Node>> = vec![];

        // SAFETY: from the DTSpec we know this is a depth-first traversal
        // we also know that for each node the FDT representation starts with the node name
        // followed by properties and ends with the children nodes; thus we can
        // safely assume that the parent nodes in the stack have properties defined
        for entry in fdt.structure_block_iter() {
            let entry = entry?;

            match entry.kind() {
                StructureBlockEntryKind::BeginNode(node) => {
                    let name = node.name().to_string();

                    let parent = nodes_stack
                        .last()
                        .map(|parent_box| NonNull::from(parent_box.as_ref()));

                    let node = Box::new(Node::new(name, parent));

                    nodes_stack.push(node);
                }
                StructureBlockEntryKind::EndNode => {
                    let child = nodes_stack.pop().ok_or(())?;

                    if let Some(parent) = nodes_stack.last_mut() {
                        parent.children_mut().push(child);
                    } else {
                        return Ok(DeviceTree {
                            address,
                            size,
                            version,
                            last_comp_version,
                            boot_cpuid_phys,
                            memory_reservations,
                            root: child,
                        });
                    }
                }
                StructureBlockEntryKind::Prop(prop) => {
                    if let Some(current_node) = nodes_stack.last_mut() {
                        let property = Property::from_raw(prop, &current_node);
                        current_node.properties_mut().push(property);
                    }
                }
            };
        }

        Err(())
    }
}
