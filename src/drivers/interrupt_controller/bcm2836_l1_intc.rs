use alloc::boxed::Box;

use crate::drivers::DriverInitError::DeviceTreeError;
use crate::drivers::{DriverInitError, PlatformDriver};
use crate::kernel::devicetree::interrupts::{
    InterruptControllerNode, InterruptControllerOrNexusNode,
};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::irq::{InterruptController, IrqResult, register_controller};
use crate::kprintln;
use crate::mm::map_io_region;

mod hwirq {
    pub const COUNT: u32 = 12;
    // timer interrupts
    pub const HWIRQ_CNTPSIRQ: u32 = 0;
    pub const HWIRQ_CNTPNSIRQ: u32 = 1;
    pub const HWIRQ_CNTHPIRQ: u32 = 2;
    pub const HWIRQ_CNTVIRQ: u32 = 3;
    // mailbox interrupts
    pub const HWIRQ_MAILBOX_0: u32 = 4;
    pub const HWIRQ_MAILBOX_1: u32 = 5;
    pub const HWIRQ_MAILBOX_2: u32 = 6;
    pub const HWIRQ_MAILBOX_3: u32 = 7;
    // GPU interrupt
    pub const HWIRQ_GPU: u32 = 8;
    // PMU (performance monitoring unit) interrupt
    pub const HWIRQ_PMU: u32 = 9;
    // AXI interrupt
    pub const HWIRQ_AXI: u32 = 10;
    // Local timer interrupt
    pub const HWIRQ_LOCAL_TIMER: u32 = 11;
}

// Core related interrupts
const CORE_RELATED_DEST_NONE: u32 = 0;
const CORE_RELATED_DEST_IRQ: u32 = 1;
const CORE_RELATED_DEST_FIQ: u32 = 2;

// Core un-related interrupts
const CORE_UNRELATED_DEST_IRQ_CORE_0: u32 = 0;
const CORE_UNRELATED_DEST_IRQ_CORE_1: u32 = 1;
const CORE_UNRELATED_DEST_IRQ_CORE_2: u32 = 2;
const CORE_UNRELATED_DEST_IRQ_CORE_3: u32 = 3;
const CORE_UNRELATED_DEST_FIQ_CORE_0: u32 = 4;
const CORE_UNRELATED_DEST_FIQ_CORE_1: u32 = 5;
const CORE_UNRELATED_DEST_FIQ_CORE_2: u32 = 6;
const CORE_UNRELATED_DEST_FIQ_CORE_3: u32 = 7;

// Registers
const CORE_IRQ_SOURCE_REG_OFFSET: usize = 0x60;
const CORE_FIQ_SOURCE_REG_OFFSET: usize = 0x70;
const CORE_TIMERS_INT_CTRL_REG_OFFSET: usize = 0x40;
const CORE_MBOX_INT_CTRL_REG_OFFSET: usize = 0x50;
const GPU_INT_ROUTING_REG_OFFSET: usize = 0xc;

pub struct InterruptControllerDriver;

impl InterruptControllerDriver {
    pub const fn new() -> Self {
        Self {}
    }
}

pub struct InterruptControllerDevice {
    reg_base: usize,
}

impl InterruptControllerDevice {
    pub fn init(reg_base: usize) -> Self {

        // TODO: missing actual initialization of the interrupt controller hardware
        
        Self { reg_base }
    }
}

impl InterruptController for InterruptControllerDevice {
    fn enable(&self, hwirq: u32) {
        todo!()
    }

    fn disable(&self, hwirq: u32) {
        todo!()
    }

    fn pending_hwirq(&self) -> Option<u32> {
        todo!()
    }

    fn xlate(&self, specifier: &[u32]) -> IrqResult<u32> {
        todo!()
    }
}

impl PlatformDriver for InterruptControllerDriver {
    fn compatible(&self) -> &str {
        "brcm,bcm2836-l1-intc"
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        kprintln!("[{}] try init", self.compatible());

        if !node.is_interrupt_controller() {
            kprintln!(
                "[ERROR][{}] missing 'interrupt-controller' property",
                self.compatible()
            );
            return Err(DeviceTreeError);
        }

        let interrupt_cells = node.interrupt_cells().ok_or(DeviceTreeError)?;
        if interrupt_cells != 2 {
            kprintln!(
                "[ERROR][{}] invalid '#interrupt-cells' property value: expected 2, got {}",
                self.compatible(),
                interrupt_cells
            );
            return Err(DeviceTreeError);
        }

        let reg = node.reg().ok_or(DeviceTreeError)?;
        if reg.len() != 1 {
            kprintln!(
                "[ERROR][{}] invalid 'reg' property: expected 1 region, got {}",
                self.compatible(),
                reg.len()
            );
            return Err(DeviceTreeError);
        }

        let reg = reg.first().ok_or(DeviceTreeError)?;
        let phys_addr = reg.address_as_usize().map_err(|_| DeviceTreeError)?;
        let length = reg.length_as_usize().map_err(|_| DeviceTreeError)?;

        kprintln!(
            "[{}] reg: phys_addr=0x{:x}, length=0x{:x}",
            self.compatible(),
            phys_addr,
            length
        );

        let addr = map_io_region(phys_addr, length);
        let dev = InterruptControllerDevice::init(addr);
        
        register_controller(node, Box::new(dev), hwirq::COUNT)
            .map_err(|_| DriverInitError::Retry)?;

        kprintln!("[{}] initialized successfully", self.compatible());

        Ok(())
    }
}

pub static DRIVER: InterruptControllerDriver = InterruptControllerDriver::new();
