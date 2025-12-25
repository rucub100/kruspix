use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::kernel::devicetree::prop::Property;
use crate::kernel::sync::OnceLock;

use crate::kernel::devicetree::std_prop::PHANDLE;
use fdt::{Fdt, fdt_structure_block::StructureBlockEntryKind};
use node::Node;

pub mod fdt;
pub mod interrupts;
pub mod node;
pub mod prop;
pub mod std_prop;
pub mod misc_prop;

static FDT_ADDR: AtomicUsize = AtomicUsize::new(0);
static DEVICE_TREE: OnceLock<Arc<DeviceTree>> = OnceLock::new();

pub fn register_fdt_addr(addr: usize) {
    FDT_ADDR
        .compare_exchange(0, addr, Ordering::SeqCst, Ordering::SeqCst)
        .unwrap();
}

pub fn init_devicetree() {
    let fdt = unsafe { Fdt::new(FDT_ADDR.load(Ordering::Relaxed)).expect("Failed to parse FDT") };
    let device_tree = DeviceTree::from_fdt(&fdt).expect("Failed to convert FDT to DeviceTree");

    assert!(device_tree.version() >= 17);
    assert_eq!(device_tree.last_compatible_version(), 16);

    DEVICE_TREE
        .set(Arc::new(device_tree))
        .expect("DeviceTree already initialized");
}

pub fn get_devicetree() -> Option<Arc<DeviceTree>> {
    DEVICE_TREE.get().cloned()
}

#[derive(Debug)]
pub struct DeviceTree {
    address: usize,
    size: u32,
    version: u32,
    last_comp_version: u32,
    boot_cpuid_phys: u32,
    memory_reservations: Vec<(usize, usize)>,
    root: Box<Node>,
    phandle_map: BTreeMap<u32, *const Node>,
}

// SAFETY: DeviceTree is immutable after initialization. 
// The phandle_map contains pointers to Nodes that are owned by the 'root' Box,
// meaning their heap addresses are stable for the lifetime of the DeviceTree.
unsafe impl Send for DeviceTree {}
unsafe impl Sync for DeviceTree {}

impl DeviceTree {
    fn from_fdt(fdt: &Fdt) -> Result<Self, ()> {
        let address = fdt.address();
        let size = fdt.size();
        let version = fdt.version();
        let last_comp_version = fdt.last_compatible_version();
        let boot_cpuid_phys = fdt.boot_cpuid_phys();
        let memory_reservations: Vec<_> = fdt
            .memory_reservation_block_iter()
            .map(|r| (r.address() as usize, r.size() as usize))
            .collect();

        let mut nodes_stack: Vec<Box<Node>> = vec![];
        let mut phandle_map = BTreeMap::new();

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
                            phandle_map,
                        });
                    }
                }
                StructureBlockEntryKind::Prop(prop) => {
                    if let Some(current_node) = nodes_stack.last_mut() {
                        let property = Property::from_raw(prop, &current_node);

                        if property.name() == PHANDLE {
                            if let Ok(phandle) = prop.value_as_phandle() {
                                phandle_map.insert(phandle, current_node.as_ref() as *const Node);
                            }
                        }

                        current_node.properties_mut().push(property);
                    }
                }
            };
        }

        Err(())
    }

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

    pub fn node_by_phandle(&self, phandle: &PHandle) -> Option<&Node> {
        let ptr = self.phandle_map.get(&phandle.0)?;
        // SAFETY: nodes are boxed and the devicetree is protected by an Arc,
        // so the pointer is valid as long as the devicetree exists (which is forever)
        unsafe { ptr.as_ref() }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct PHandle(pub u32);
