// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::string::String;
use alloc::sync::Arc;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::console::{Console, register_early_console, register_console};
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::devicetree::{fdt::Fdt, node::Node};
use crate::kprintln;
use crate::mm::map_io_region;

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

struct MiniUartDevice {
    id: String,
    reg_base: usize,
}

impl MiniUartDevice {
    fn new(id: String, reg_base: usize) -> Self {
        Self { id, reg_base }
    }
}

impl Device for MiniUartDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        let skip_init = node
            .properties()
            .iter()
            .any(|prop| prop.name() == "skip-init");
        if skip_init {
            kprintln!("[INFO] Mini UART initialization skipped via 'skip-init' property");
        } else {
            todo!();
        }

        register_console(self.clone());

        // TODO: register the interrupt handler
        // what about input?

        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl Console for MiniUartDevice {
    fn write(&self, s: &str) {
        let aux_mu_lsr_reg = (self.reg_base + AUX_MU_LSR_REG_OFFSET) as *mut u32;
        let aux_mu_io_reg = (self.reg_base + AUX_MU_IO_REG_OFFSET) as *mut u32;

        let write_byte = |byte: u8| unsafe {
            loop {
                if (read_volatile(aux_mu_lsr_reg) & TX_EMPTY) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }

            write_volatile(aux_mu_io_reg, byte as u32);
        };

        for byte in s.bytes() {
            if byte == b'\n' {
                write_byte(b'\r');
            }
            write_byte(byte);
        }
    }
}

pub struct MiniUartDriver {
    early_reg_base: AtomicUsize,
    dev_registry: DriverRegistry<MiniUartDevice>,
}

impl MiniUartDriver {
    const fn new() -> Self {
        Self {
            early_reg_base: AtomicUsize::new(0),
            dev_registry: DriverRegistry::new(),
        }
    }
}

// Implement Device for MiniUartDriver to satisfy the trait requirements.
impl Device for MiniUartDriver {
    fn id(&self) -> &str {
        "brcm,bcm2835-aux-uart"
    }

    fn global_setup(self: Arc<Self>, _node: &Node) -> Result<(), DriverInitError> {
        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl Console for MiniUartDriver {
    /// Write a string to the mini UART.
    /// # Safety
    /// This function is only suitable for early console output during boot.
    fn write(&self, s: &str) {
        let reg_base = self.early_reg_base.load(Ordering::Acquire);
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

impl PlatformDriver for MiniUartDriver {
    fn compatible(&self) -> &[&str] {
        &["brcm,bcm2835-aux-uart"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        kprintln!("{:?} try init", self.compatible());

        let reg = node.reg().ok_or(DriverInitError::DeviceTreeError)?;
        if reg.len() != 1 {
            kprintln!(
                "[ERROR]{:?} invalid 'reg' property: expected 1 region, got {}",
                self.compatible(),
                reg.len()
            );
            return Err(DriverInitError::DeviceTreeError);
        }

        let (phys_addr, length) = node
            .resolve_phys_address_and_length(0)
            .ok_or(DriverInitError::DeviceTreeError)?;

        let addr = map_io_region(phys_addr, length);
        let dev = MiniUartDevice::new(node.path(), addr);
        let dev = Arc::new(dev);
        self.dev_registry.add_device(node.path(), dev.clone());

        dev.clone().global_setup(node)?;

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }

    fn early_init(&'static self, fdt: &Fdt, path: &str) {
        if let Some(addr) = fdt.resolve_phys_addr(path) {
            if self
                .early_reg_base
                .compare_exchange(0, addr, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                register_early_console(self);
            }
        }
    }
}

pub static DRIVER: MiniUartDriver = MiniUartDriver::new();
