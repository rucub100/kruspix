use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::drivers::{DriverInitError, PlatformDriver};
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

const TX_EMPTY: u32 = 1 << 5;

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
    /// Write a string to the mini UART.
    /// # Safety
    /// This function is only suitable for early console output during boot.
    fn write(&self, s: &str) {
        let reg_base = self.reg_base.load(Ordering::Acquire);
        let aux_mu_lsr_reg = (reg_base + AUX_MU_LSR_REG_OFFSET) as *mut u32;
        let aux_mu_io_reg = (reg_base + AUX_MU_IO_REG_OFFSET) as *mut u32;
        let write_byte = |byte: u8| unsafe {
            // the write before wait is preferred in this early console scenario
            write_volatile(aux_mu_io_reg, byte as u32);

            // wait until transmit can accept another byte
            loop {
                if (read_volatile(aux_mu_lsr_reg) & TX_EMPTY) != 0 {
                    break;
                }
                core::hint::spin_loop();
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
    fn write(&self, _s: &str) {
        todo!()
    }
}

impl PlatformDriver for MiniUartDriver {
    fn compatible(&self) -> &str {
        "brcm,bcm2835-aux-uart"
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        // TODO:
        // 1. initialize the hardware
        // 2. map the physical register address to IO PERIPHERAL space
        // 3. register the MiniUartDevice instance as a console device (capability)

        // TODO: more considerations:
        // - analyze the device bindings (properties) and handle all of them
        //   + reg => map physical address to IO PERIPHERAL space
        //   + status => check if "okay"
        //   + phandle => register the phandle for other devices to reference?! (this is not task of driver but rather of devicetree manager)
        //   + skip-init => skip initialization if present (but still provide init functionality and test it)
        //   + pinctrl-0 and pinctrl-names => figure what how to handle this (probably pin controller driver should handle this)
        //   + clocks => request clock via subsystem and enable it
        //   + interrupts => register handler for the interrupts for async operation without polling
        // - this device driver seems to depend on other subsystems (clock, pin controller, interrupt controller)
        //   therefore it is probably a good idea to start with the most essential dependencies first

        for prop in node.properties() {
            kprintln!("-> Property: {}", prop.name());
        }

        Err(DriverInitError::ToDo)
    }

    fn early_init(&'static self, fdt: &Fdt, path: &str) {
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
