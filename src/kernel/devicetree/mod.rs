use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::iter;
use core::ops::Deref;
use core::ptr::NonNull;

use super::boot::{
    devicetree::{
        Fdt,
        fdt_structure_block::StructureBlockEntryKind,
        prop::{Prop, StandardProp},
    },
    sync::BootCell,
};

static FDT: BootCell<Fdt> = BootCell::new();

pub fn set_fdt(fdt: Fdt) {
    FDT.init(fdt);
}

pub fn get_devicetree() -> Option<DeviceTree> {
    FDT.try_lock()?.deref().try_into().ok()
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

        let mut nodes: Vec<Box<Node>> = vec![];

        for entry in fdt.structure_block_iter() {
            let entry = entry?;

            match entry.kind() {
                StructureBlockEntryKind::BeginNode(node) => {
                    let name = node.name().to_str().map_err(|_| ())?.to_string();

                    let parent = nodes
                        .last()
                        .map(|parent_box| NonNull::from(parent_box.as_ref()));

                    let node = Box::new(Node {
                        name,
                        properties: vec![],
                        children: vec![],
                        parent,
                    });

                    nodes.push(node);
                }
                StructureBlockEntryKind::EndNode => {
                    let child = nodes.pop().ok_or(())?;

                    if let Some(parent) = nodes.last_mut() {
                        parent.children.push(child);
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
                    if let Some(current_node) = nodes.last_mut() {
                        current_node.properties.push(prop.try_into()?);
                    }
                }
            };
        }

        Err(())
    }
}

pub struct Node {
    name: String,
    properties: Vec<Property>,
    children: Vec<Box<Node>>,
    parent: Option<NonNull<Node>>,
}

impl Node {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn node_name(&self) -> &str {
        &self.name.split('@').next().unwrap_or(&self.name)
    }

    pub fn unit_address(&self) -> Option<&str> {
        let parts: Vec<&str> = self.name.split('@').collect();
        if parts.len() > 1 {
            Some(parts[1])
        } else {
            None
        }
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    pub fn is_compatible_with(&self, compatible: &str) -> bool {
        let compatible_prop = self.properties.iter().find(|p| p.name() == "compatible");
        if let Some(prop) = compatible_prop {
            return match &prop.value {
                PropertyValue::Standard(StandardProperty::Compatible(c)) => {
                    c.iter().any(|s| s == compatible)
                }
                _ => false,
            };
        }

        false
    }

    pub fn path(&self) -> String {
        let mut segments = Vec::with_capacity(16);
        let mut current = Some(self);

        while let Some(node) = current {
            if !node.name.is_empty() {
                segments.push(node.name.as_str());
            }
            current = node.parent();
        }

        if segments.is_empty() {
            return String::from("/");
        }

        let path_len = segments.iter().map(|s| s.len() + 1).sum();
        let mut path = String::with_capacity(path_len);

        segments.iter().rev().for_each(|name| {
            path.push('/');
            path.push_str(name);
        });

        path
    }

    pub fn properties(&self) -> &Vec<Property> {
        &self.properties
    }

    pub fn children(&self) -> &Vec<Box<Node>> {
        &self.children
    }

    pub fn parent(&self) -> Option<&Node> {
        self.parent.map(|p| unsafe { p.as_ref() })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        let mut stack = Vec::new();
        stack.push(self);

        iter::from_fn(move || {
            let node = stack.pop()?;
            stack.extend(node.children.iter().rev().map(|b| b.as_ref()));
            Some(node)
        })
    }
}

#[derive(Debug)]
pub enum PropertyValue {
    Standard(StandardProperty),
    Other(PropertyValueType),
}

#[derive(Debug)]
pub struct Property {
    name: String,
    value: PropertyValue,
}

impl Property {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &PropertyValue {
        &self.value
    }
}

impl TryFrom<&Prop> for Property {
    type Error = ();

    fn try_from(prop: &Prop) -> Result<Self, Self::Error> {
        let mut value: PropertyValue =
            PropertyValue::Other(PropertyValueType::PropEncodedArray(prop.value().to_vec()));

        let standard_prop: Result<StandardProp, ()> = prop.name().to_bytes().try_into();
        if let Ok(standard_prop) = standard_prop {
            value = match standard_prop {
                StandardProp::Compatible => PropertyValue::Standard(StandardProperty::Compatible(
                    prop.value_as_string_list_iter()
                        .filter_map(|res| {
                            res.ok()
                                .and_then(|cstr| cstr.to_str().ok())
                                .map(|s| s.to_string())
                        })
                        .collect(),
                )),
                StandardProp::Model => PropertyValue::Standard(StandardProperty::Model(
                    prop.value_as_string()?
                        .to_str()
                        .map_err(|_| ())?
                        .to_string(),
                )),
                StandardProp::PHandle => {
                    PropertyValue::Standard(StandardProperty::PHandle(prop.value_as_u32()?))
                }
                StandardProp::Status => PropertyValue::Standard(StandardProperty::Status(
                    prop.value_as_string()?
                        .to_str()
                        .map_err(|_| ())?
                        .to_string(),
                )),
                StandardProp::AddressCells => {
                    PropertyValue::Standard(StandardProperty::AddressCells(prop.value_as_u32()?))
                }
                StandardProp::SizeCells => {
                    PropertyValue::Standard(StandardProperty::SizeCells(prop.value_as_u32()?))
                }
                // we cannot parse the value here because we don't know the #address-cells and #size-cells of the parent node
                // StandardProp::Reg => PropertyValue::Standard(StandardProperty::Reg(prop.value_as_prop_encoded_array_cells_pair_iter(/* u32 */, /* u32 */).collect())),
                StandardProp::VirtualReg => PropertyValue::Standard(StandardProperty::VirtualReg(
                    prop.value_as_u32()? as usize,
                )),
                // we cannot parse the value here because we don't know the #address-cells and #size-cells of the parent node
                // StandardProp::Ranges => PropertyValue::Standard(StandardProperty::Ranges(prop.value_as_optional_prop_encoded_array_cells_triple_iter(/* u32 */, /* u32 */, /* u32 */).collect())),
                // StandardProp::DmaRanges => PropertyValue::Standard(StandardProperty::DmaRanges(prop.value_as_optional_prop_encoded_array_cells_triple_iter(/* u32 */, /* u32 */, /* u32 */).collect())),
                StandardProp::DmaCoherent => PropertyValue::Standard(StandardProperty::DmaCoherent),
                StandardProp::DmaNoncoherent => {
                    PropertyValue::Standard(StandardProperty::DmaNoncoherent)
                }
                _ => value,
            }
        }

        Ok(Self {
            name: prop.name().to_str().map_err(|_| ())?.to_string(),
            value,
        })
    }
}

#[derive(Debug)]
pub enum PropertyValueType {
    Empty,
    U32(u32),
    U64(u64),
    String(String),
    PropEncodedArray(Vec<u8>),
    PHandle(u32),
    StringList(Vec<String>),
}

pub enum StatusValue {
    Ok,
    Disabled,
    Reserved,
    Fail(String),
}

#[derive(Debug)]
pub enum StandardProperty {
    Compatible(Vec<String>),
    Model(String),
    PHandle(u32),
    Status(String),
    AddressCells(u32),
    SizeCells(u32),
    Reg(Vec<(usize, usize)>),
    VirtualReg(usize),
    Ranges(Option<Vec<(usize, usize, usize)>>),
    DmaRanges(Option<Vec<(usize, usize, usize)>>),
    DmaCoherent,
    DmaNoncoherent,
    // deprecated properties
    Name(String),
    DeviceType(String),
}

impl StandardProperty {
    pub const COMPATIBLE: &'static str = "compatible";
    pub const MODEL: &'static str = "model";
    pub const P_HANDLE: &'static str = "phandle";
    pub const STATUS: &'static str = "status";
    pub const ADDRESS_CELLS: &'static str = "#address-cells";
    pub const SIZE_CELLS: &'static str = "#size-cells";
    pub const REG: &'static str = "reg";
    pub const VIRTUAL_REG: &'static str = "virtual-reg";
    pub const RANGES: &'static str = "ranges";
    pub const DMA_RANGES: &'static str = "dma-ranges";
    pub const DMA_COHERENT: &'static str = "dma-coherent";
    pub const DMA_NONCOHERENT: &'static str = "dma-noncoherent";

    pub fn name(&self) -> &str {
        match self {
            StandardProperty::Compatible(_) => Self::COMPATIBLE,
            StandardProperty::Model(_) => Self::MODEL,
            StandardProperty::PHandle(_) => Self::P_HANDLE,
            StandardProperty::Status(_) => Self::STATUS,
            StandardProperty::AddressCells(_) => Self::ADDRESS_CELLS,
            StandardProperty::SizeCells(_) => Self::SIZE_CELLS,
            StandardProperty::Reg(_) => Self::REG,
            StandardProperty::VirtualReg(_) => Self::VIRTUAL_REG,
            StandardProperty::Ranges(_) => Self::RANGES,
            StandardProperty::DmaRanges(_) => Self::DMA_RANGES,
            StandardProperty::DmaCoherent => Self::DMA_COHERENT,
            StandardProperty::DmaNoncoherent => Self::DMA_NONCOHERENT,
            // deprecated properties
            StandardProperty::Name(_) => "name",
            StandardProperty::DeviceType(_) => "device_type",
        }
    }
}
