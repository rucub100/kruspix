use core::sync::atomic::{AtomicUsize, Ordering};

use crate::drivers::PlatformDriver;
use crate::kernel::devicetree::Node;

/// I/O Data (8 bits)
const AUX_MU_IO_REG_OFFSET: usize = 0x40;
/// Interrupt Enable (8 bits)
const AUX_MU_IER_REG_OFFSET: usize = 0x44;
/// Interrupt Identify (8 bits)
const AUX_MU_IIR_REG_OFFSET: usize = 0x48;
/// Line Control (8 bits)
const AUX_MU_LCR_REG_OFFSET: usize = 0x4C;
/// Modem Control (8 bits)
const AUX_MU_MCR_REG_OFFSET: usize = 0x50;
/// Line Status (8 bits)
const AUX_MU_LSR_REG_OFFSET: usize = 0x54;
/// Modem Status (8 bits)
const AUX_MU_MSR_REG_OFFSET: usize = 0x58;
/// Scratch (8 bits)
const AUX_MU_SCRATCH_OFFSET: usize = 0x5C;
/// Extra Control (8 bits)
const AUX_MU_CNTL_REG_OFFSET: usize = 0x60;
/// Extra Status (32 bits)
const AUX_MU_STAT_REG_OFFSET: usize = 0x64;
/// Baudrate (16 bits)
const AUX_MU_BAUD_REG_OFFSET: usize = 0x68;

static AUX_BASE: AtomicUsize = AtomicUsize::new(0);

pub struct MiniUart;

impl MiniUart {}

impl PlatformDriver for MiniUart {
    fn compatible(&self) -> &str {
        "brcm,bcm2835-aux-uart"
    }

    fn init(&self, node: &Node) {
        // TODO:
        // 1. initialize the hardware
        // 2. we may have to map the MMIO region from the device tree node
        // 3. register the UART as a console device (capability)
        AUX_BASE.store(0, Ordering::Release);
    }
}

pub static DRIVER: MiniUart = MiniUart;
