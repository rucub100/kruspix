use alloc::vec::Vec;

use super::PHandle;

pub trait InterruptGeneratingDevice {
    fn interrupts(&self) -> Option<&Interrupts>;
    fn interrupts_extended(&self) -> Option<&ExtendedInterrupts>;
    fn interrupt_parent(&self) -> Option<&PHandle>;
}

pub trait InterruptController {
    fn interrupt_cells(&self) -> u32;
    fn is_interrupt_controller(&self) -> bool;
}

pub trait InterruptNexus {
    fn interrupt_cells(&self) -> u32;
    fn interrupt_map(&self) -> Option<&InterruptMap>;
    fn interrupt_map_mask(&self) -> Option<&InterruptMapMask>;
}

pub type Interrupts = Vec<InterruptSpecifier>;

#[repr(transparent)]
pub struct InterruptSpecifier(pub Vec<u32>);

pub type ExtendedInterrupts = Vec<(PHandle, InterruptSpecifier)>;

pub struct InterruptMap {
    child_unit_addr: Vec<u32>,
    child_interrupt_specifier: InterruptSpecifier,
    interrupt_parent: PHandle,
    parent_unit_addr: Vec<u32>,
    parent_interrupt_specifier: InterruptSpecifier,
}

#[repr(transparent)]
pub struct InterruptMapMask(pub Vec<u32>);