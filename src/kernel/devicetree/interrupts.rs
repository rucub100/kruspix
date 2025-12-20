use alloc::vec::Vec;
use core::iter;

use super::PHandle;

pub const INTERRUPTS: &'static str = "interrupts";
pub const INTERRUPTS_EXTENDED: &'static str = "interrupts-extended";
pub const INTERRUPT_PARENT: &'static str = "interrupt-parent";
pub const INTERRUPT_CELLS: &'static str = "#interrupt-cells";
pub const INTERRUPT_CONTROLLER: &'static str = "interrupt-controller";
pub const INTERRUPT_MAP: &'static str = "interrupt-map";
pub const INTERRUPT_MAP_MASK: &'static str = "interrupt-map-mask";

pub trait InterruptGeneratingNode {
    fn interrupts(&self) -> Option<&Interrupts>;
    fn interrupts_extended(&self) -> Option<&ExtendedInterrupts>;
    fn interrupt_parent(&self) -> Option<&PHandle>;
}

pub trait InterruptControllerNode {
    fn is_interrupt_controller(&self) -> bool;
}

pub trait InterruptNexusNode {
    fn interrupt_map(&self) -> Option<&InterruptMap>;
    fn interrupt_map_mask(&self) -> Option<&InterruptMapMask>;
}

pub trait InterruptControllerOrNexusNode: InterruptControllerNode + InterruptNexusNode {
    fn interrupt_cells(&self) -> Option<u32>;
}

#[derive(Debug)]
pub struct Interrupts {
    raw_data: Vec<u32>,
}

impl Interrupts {
    pub fn from_raw(data: Vec<u32>) -> Self {
        Self { raw_data: data }
    }

    pub fn iter(&self, interrupt_cells: u32) -> impl Iterator<Item = InterruptSpecifier> {
        assert!(interrupt_cells > 0);

        self.raw_data
            .chunks_exact(interrupt_cells as usize)
            .map(|chunk| InterruptSpecifier(chunk.to_vec()))
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct InterruptSpecifier(pub Vec<u32>);

#[derive(Debug)]
pub struct ExtendedInterrupts {
    raw_data: Vec<u32>,
}

type ExtendedSpecifier = (PHandle, InterruptSpecifier);

impl ExtendedInterrupts {
    pub fn from_raw(data: Vec<u32>) -> Self {
        Self { raw_data: data }
    }

    pub fn iter(&self, _interrupt_cells: &[usize]) -> impl Iterator<Item = ExtendedSpecifier> {
        todo!();
        iter::empty()
    }
}

pub struct InterruptMapEntry {
    child_unit_addr: Vec<u32>,
    child_interrupt_specifier: InterruptSpecifier,
    interrupt_parent: PHandle,
    parent_unit_addr: Vec<u32>,
    parent_interrupt_specifier: InterruptSpecifier,
}

#[derive(Debug)]
pub struct InterruptMap {
    raw_data: Vec<u32>,
}

impl InterruptMap {
    pub fn from_raw(data: Vec<u32>) -> Self {
        Self { raw_data: data }
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct InterruptMapMask(pub Vec<u32>);

#[derive(Debug)]
pub enum InterruptsProperty {
    Interrupts(Interrupts),
    ExtendedInterrupts(ExtendedInterrupts),
    InterruptParent(PHandle),
    InterruptCells(u32),
    InterruptController,
    InterruptMap(InterruptMap),
    InterruptMapMask(InterruptMapMask),
}

impl InterruptsProperty {
    pub fn as_str(&self) -> &str {
        match self {
            InterruptsProperty::Interrupts(_) => INTERRUPTS,
            InterruptsProperty::ExtendedInterrupts(_) => INTERRUPTS_EXTENDED,
            InterruptsProperty::InterruptParent(_) => INTERRUPT_PARENT,
            InterruptsProperty::InterruptCells(_) => INTERRUPT_CELLS,
            InterruptsProperty::InterruptController => INTERRUPT_CONTROLLER,
            InterruptsProperty::InterruptMap(_) => INTERRUPT_MAP,
            InterruptsProperty::InterruptMapMask(_) => INTERRUPT_MAP_MASK,
        }
    }
}
