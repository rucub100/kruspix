use super::PHandle;
use super::interrupts::{
    ExtendedInterrupts, INTERRUPT_CELLS, INTERRUPT_CONTROLLER, INTERRUPT_MAP, INTERRUPT_MAP_MASK,
    INTERRUPT_PARENT, INTERRUPTS, INTERRUPTS_EXTENDED, InterruptControllerOrNexusNode,
    InterruptMap, InterruptMapMask, Interrupts, InterruptsProperty,
};
use super::interrupts::{InterruptControllerNode, InterruptGeneratingNode, InterruptNexusNode};
use super::prop::{Property, PropertyValue};
use super::std_prop::StandardProperties;
use super::std_prop::{
    ADDRESS_CELLS, COMPATIBLE, DMA_COHERENT, DMA_NONCOHERENT, DMA_RANGES, MODEL, PHANDLE, RANGES,
    REG, SIZE_CELLS, STATUS, VIRTUAL_REG,
};
use super::std_prop::{
    AddressCellsValue, CompatibleValue, DmaRangesValue, RangesValue, RegValue, SizeCellsValue,
    StandardProperty, StatusValue,
};
use crate::kernel::devicetree::misc_prop::{
    CLOCK_FREQUENCY, ClockFrequency, MiscellaneousProperties, MiscellaneousProperty,
};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::iter;
use core::ptr::NonNull;

// Base Device Node Types
pub const ROOT: &'static str = "";
pub const ALIASES: &'static str = "aliases";
pub const MEMORY: &'static str = "memory";
pub const RESERVED_MEMORY: &'static str = "reserved-memory";
pub const CHOSEN: &'static str = "chosen";
pub const CHOSEN_BOOTARGS: &'static str = "bootargs";
pub const CHOSEN_STDOUT_PATH: &'static str = "stdout-path";
pub const CHOSEN_STDIN_PATH: &'static str = "stdin-path";
pub const CPUS: &'static str = "cpus";

#[derive(Debug)]
pub struct Node {
    name: String,
    properties: Vec<Property>,
    children: Vec<Box<Node>>,
    parent: Option<NonNull<Node>>,
}

impl Node {
    pub const fn new(name: String, parent: Option<NonNull<Node>>) -> Self {
        Node {
            name,
            properties: Vec::new(),
            children: Vec::new(),
            parent,
        }
    }

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

    pub(crate) fn properties_mut(&mut self) -> &mut Vec<Property> {
        &mut self.properties
    }

    pub fn children(&self) -> &Vec<Box<Node>> {
        &self.children
    }

    pub(crate) fn children_mut(&mut self) -> &mut Vec<Box<Node>> {
        &mut self.children
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

    /// # Safety
    /// We assume that address and length cells fit into usize.
    /// Furthermore, we assume that the reg length is always equal to the range length.
    pub fn resolve_phys_address_and_length(&self) -> Option<(usize, usize)> {
        let address_length_pair = self
            .reg()
            .and_then(|reg| reg.first())
            .map(|reg| (reg.address_as_usize().ok(), reg.length_as_usize().ok()));

        if let Some((address, length)) = address_length_pair
            && let (Some(address), Some(length)) = (address, length)
        {
            let mut address = address;
            let mut parent = self.parent();

            while let Some(parent_node) = parent
                && parent_node.ranges().is_some()
            {
                let range = parent_node.ranges().unwrap().iter().find(|range| {
                    let range_child_bus_addr = range.child_bus_addr_as_usize().ok();
                    let range_parent_bus_addr = range.parent_bus_addr_as_usize().ok();
                    let range_length = range.length_as_usize().ok();

                    if let (
                        Some(range_child_bus_addr),
                        Some(range_parent_bus_addr),
                        Some(range_length),
                    ) = (range_child_bus_addr, range_parent_bus_addr, range_length)
                    {
                        address >= range_child_bus_addr
                            && address < (range_child_bus_addr + range_length)
                    } else {
                        false
                    }
                });

                if range.is_none() {
                    break;
                }

                let range = range.unwrap();
                let range_child_bus_addr = range.child_bus_addr_as_usize().unwrap();
                let range_parent_bus_addr = range.parent_bus_addr_as_usize().unwrap();
                let range_length = range.length_as_usize().unwrap();

                assert!(range_length > length);

                address = range_parent_bus_addr + (address - range_child_bus_addr);

                parent = parent_node.parent();
            }

            return Some((address, length));
        }

        None
    }
}

impl StandardProperties for Node {
    fn compatible(&self) -> Option<&CompatibleValue> {
        self.properties
            .iter()
            .find(|p| p.name() == COMPATIBLE)
            .and_then(|p| match &p.value() {
                PropertyValue::Standard(StandardProperty::Compatible(compatible_list)) => {
                    Some(compatible_list)
                }
                _ => unreachable!(),
            })
    }

    fn model(&self) -> Option<&str> {
        self.properties
            .iter()
            .find(|p| p.name() == MODEL)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::Model(model)) => Some(model.as_str()),
                _ => unreachable!(),
            })
    }

    fn phandle(&self) -> Option<&PHandle> {
        self.properties
            .iter()
            .find(|p| p.name() == PHANDLE)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::PHandle(phandle)) => Some(phandle),
                _ => unreachable!(),
            })
    }

    fn status(&self) -> Option<&StatusValue> {
        self.properties
            .iter()
            .find(|p| p.name() == STATUS)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::Status(status)) => Some(status),
                _ => unreachable!(),
            })
    }

    fn address_cells(&self) -> AddressCellsValue {
        self.properties
            .iter()
            .find(|p| p.name() == ADDRESS_CELLS)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::AddressCells(addr_cells)) => {
                    Some(*addr_cells)
                }
                _ => unreachable!(),
            })
            .unwrap_or_default()
    }

    fn size_cells(&self) -> SizeCellsValue {
        self.properties
            .iter()
            .find(|p| p.name() == SIZE_CELLS)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::SizeCells(size_cells)) => {
                    Some(*size_cells)
                }
                _ => unreachable!(),
            })
            .unwrap_or_default()
    }

    fn reg(&self) -> Option<&RegValue> {
        self.properties
            .iter()
            .find(|p| p.name() == REG)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::Reg(reg)) => Some(reg),
                _ => unreachable!(),
            })
    }

    fn virtual_reg(&self) -> Option<u32> {
        self.properties
            .iter()
            .find(|p| p.name() == VIRTUAL_REG)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::VirtualReg(virt_reg)) => Some(*virt_reg),
                _ => unreachable!(),
            })
    }

    fn ranges(&self) -> Option<&RangesValue> {
        self.properties
            .iter()
            .find(|p| p.name() == RANGES)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::Ranges(ranges)) => Some(ranges),
                _ => unreachable!(),
            })
    }

    fn dma_ranges(&self) -> Option<&DmaRangesValue> {
        self.properties
            .iter()
            .find(|p| p.name() == DMA_RANGES)
            .and_then(|p| match p.value() {
                PropertyValue::Standard(StandardProperty::DmaRanges(dma_ranges)) => {
                    Some(dma_ranges)
                }
                _ => unreachable!(),
            })
    }

    fn dma_coherent(&self) -> bool {
        self.properties.iter().any(|p| p.name() == DMA_COHERENT)
    }

    fn dma_noncoherent(&self) -> bool {
        self.properties.iter().any(|p| p.name() == DMA_NONCOHERENT)
    }
}

impl InterruptGeneratingNode for Node {
    fn interrupts(&self) -> Option<&Interrupts> {
        self.properties
            .iter()
            .find(|p| p.name() == INTERRUPTS)
            .and_then(|p| match p.value() {
                PropertyValue::Interrupts(InterruptsProperty::Interrupts(interrupts)) => {
                    Some(interrupts)
                }
                _ => unreachable!(),
            })
    }

    fn interrupts_extended(&self) -> Option<&ExtendedInterrupts> {
        self.properties
            .iter()
            .find(|p| p.name() == INTERRUPTS_EXTENDED)
            .and_then(|p| match p.value() {
                PropertyValue::Interrupts(InterruptsProperty::ExtendedInterrupts(
                    ext_interrupts,
                )) => Some(ext_interrupts),
                _ => unreachable!(),
            })
    }

    fn interrupt_parent(&self) -> Option<&PHandle> {
        self.properties
            .iter()
            .find(|p| p.name() == INTERRUPT_PARENT)
            .and_then(|p| match p.value() {
                PropertyValue::Interrupts(InterruptsProperty::InterruptParent(phandle)) => {
                    Some(phandle)
                }
                _ => unreachable!(),
            })
    }
}

impl InterruptControllerNode for Node {
    fn is_interrupt_controller(&self) -> bool {
        self.properties
            .iter()
            .any(|p| p.name() == INTERRUPT_CONTROLLER)
    }
}

impl InterruptNexusNode for Node {
    fn interrupt_map(&self) -> Option<&InterruptMap> {
        self.properties
            .iter()
            .find(|p| p.name() == INTERRUPT_MAP)
            .and_then(|p| match p.value() {
                PropertyValue::Interrupts(InterruptsProperty::InterruptMap(map)) => Some(map),
                _ => unreachable!(),
            })
    }

    fn interrupt_map_mask(&self) -> Option<&InterruptMapMask> {
        self.properties
            .iter()
            .find(|p| p.name() == INTERRUPT_MAP_MASK)
            .and_then(|p| match p.value() {
                PropertyValue::Interrupts(InterruptsProperty::InterruptMapMask(map_mask)) => {
                    Some(map_mask)
                }
                _ => unreachable!(),
            })
    }
}

impl InterruptControllerOrNexusNode for Node {
    fn interrupt_cells(&self) -> Option<u32> {
        self.properties
            .iter()
            .find(|p| p.name() == INTERRUPT_CELLS)
            .and_then(|p| match p.value() {
                PropertyValue::Interrupts(InterruptsProperty::InterruptCells(cells)) => {
                    Some(*cells)
                }
                _ => unreachable!(),
            })
    }
}

impl MiscellaneousProperties for Node {
    fn clock_frequency(&self) -> Option<&ClockFrequency> {
        self.properties
            .iter()
            .find(|p| p.name() == CLOCK_FREQUENCY)
            .and_then(|p| match p.value() {
                PropertyValue::Miscellaneous(MiscellaneousProperty::ClockFrequency(
                    clock_frequency,
                )) => Some(clock_frequency),
                _ => unreachable!(),
            })
    }
}

unsafe impl Send for Node {}
unsafe impl Sync for Node {}
