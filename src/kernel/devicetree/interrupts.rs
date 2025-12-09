use alloc::vec::Vec;

pub trait InterruptGeneratingDevice {
    fn interrupts(&self) -> Option<Vec<&[u8]>>;
    fn interrupts_extended(&self) -> Option<Vec<(&Self, &[u8])>>;
    fn interrupt_parent(&self) -> Option<&Self>;
}

pub trait InterruptController {
    fn interrupt_cells(&self) -> u32;
    fn is_interrupt_controller(&self) -> bool;
}

pub trait InterruptNexus {
    // TODO: define methods for interrupt nexus
    // interrupt-map, interrupt-map-mask, interrupt-cells
}

// TODO: reason about how to describe generic nexus nodes