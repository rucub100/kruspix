use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::drivers::PlatformDriver;
use crate::kernel::console::{Console, register_early_console};
use crate::kernel::devicetree::{fdt::Fdt, node::Node};
use crate::kprintln;

/// I/O Data (8 bits)
const AUX_MU_IO_REG_OFFSET: usize = 0x0;
/// Interrupt Enable (8 bits)
const AUX_MU_IER_REG_OFFSET: usize = 0x4;
/// Interrupt Identify (8 bits)
const AUX_MU_IIR_REG_OFFSET: usize = 0x8;
/// Line Control (8 bits)
const AUX_MU_LCR_REG_OFFSET: usize = 0xC;
/// Modem Control (8 bits)
const AUX_MU_MCR_REG_OFFSET: usize = 0x10;
/// Line Status (8 bits)
const AUX_MU_LSR_REG_OFFSET: usize = 0x14;
/// Modem Status (8 bits)
const AUX_MU_MSR_REG_OFFSET: usize = 0x18;
/// Scratch (8 bits)
const AUX_MU_SCRATCH_OFFSET: usize = 0x1C;
/// Extra Control (8 bits)
const AUX_MU_CNTL_REG_OFFSET: usize = 0x20;
/// Extra Status (32 bits)
const AUX_MU_STAT_REG_OFFSET: usize = 0x24;
/// Baudrate (16 bits)
const AUX_MU_BAUD_REG_OFFSET: usize = 0x28;

const TX_EMPTY: u8 = 1 << 5;

pub struct MiniUartDriver {
    reg_base: AtomicUsize,
}

impl MiniUartDriver {
    const fn new() -> Self {
        Self {
            reg_base: AtomicUsize::new(0),
        }
    }
}

impl Console for MiniUartDriver {
    fn write(&self, s: &str) {
        let reg_base = self.reg_base.load(Ordering::Acquire);
        let aux_mu_lsr_reg = (reg_base + AUX_MU_LSR_REG_OFFSET) as *mut u8;
        let aux_mu_io_reg = (reg_base + AUX_MU_IO_REG_OFFSET) as *mut u8;
        let write_byte = |byte: u8| {
            while unsafe { read_volatile(aux_mu_lsr_reg) } & TX_EMPTY == 0 {
                core::hint::spin_loop();
            }

            unsafe {
                write_volatile(aux_mu_io_reg, byte);
            }
        };

        for byte in s.bytes() {
            if byte == b'\n' {
                write_byte(b'\r');
            }
            write_byte(byte);
        }
    }
}

struct MiniUartDevice {
    reg_base: usize,
}

impl Console for MiniUartDevice {
    fn write(&self, s: &str) {
        todo!()
    }
}

impl PlatformDriver for MiniUartDriver {
    fn compatible(&self) -> &str {
        "brcm,bcm2835-aux-uart"
    }

    fn init(&self, node: &Node) {
        // TODO:
        // 1. initialize the hardware
        // 2. we may have to map the MMIO region from the device tree node
        // 3. register the UART as a console device (capability)
        // FIXME: the driver has to be a factory for multiple instances of the device
        // so a global static variable is a bad idea
        // 1 driver <-> N devices
        for prop in node.properties() {
            kprintln!("Property: {}", prop.name());
        }
    }

    fn static_init(&'static self, fdt: &Fdt, path: &str) {
        if let Some(addr) = fdt.resolve_phys_addr(path) {
            if self
                .reg_base
                .compare_exchange(0, addr, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                register_early_console(self);
            }
        }
    }
}

pub static DRIVER: MiniUartDriver = MiniUartDriver::new();
