// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! BCM2835 Random Number Generator (RNG) driver.
//!
//! This driver was developed using the following Linux kernel sources as a technical
//! reference for hardware register layouts and sequencing:
//!
//! * <https://github.com/raspberrypi/linux/blob/rpi-6.12.y/drivers/char/hw_random/bcm2835-rng.c>
//!
//! While the Linux kernel is licensed under GPL-2.0, this independent implementation
//! is provided under the MIT License. It does not contain code copied from the
//! reference sources but utilizes the hardware specifications derived from them.

use alloc::string::String;
use alloc::sync::Arc;
use core::ptr::{read_volatile, write_volatile};
use core::time::Duration;

use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::rng::{RandomNumberGenerator, RngResult, register_rng};
use crate::kernel::sync::SpinLock;
use crate::kernel::time::busy_wait;
use crate::kprintln;
use crate::mm::map_io_region;

const RNG_CTRL_REG_OFFSET: usize = 0x00;
const RNG_STATUS_REG_OFFSET: usize = 0x04;
const RNG_DATA_REG_OFFSET: usize = 0x08;

const RNG_RBGEN_BIT: u32 = 1 << 0;

const UNIT_SIZE: usize = size_of::<u32>();
const RNG_FIFO_SIZE: usize = 4;
const RNG_WAIT_PER_UNIT: Duration = Duration::from_micros(34);

const RNG_WARMUP_COUNT: u32 = 0x40000;

const UNSUPPORTED_PROPS: [&str; 4] = ["clocks", "clock-names", "resets", "reset-names"];

pub struct RngDevice {
    id: String,
    reg_base: usize,
    lock: SpinLock<()>,
}

impl RngDevice {
    const fn new(id: String, reg_base: usize) -> Self {
        RngDevice {
            id,
            reg_base,
            lock: SpinLock::new(()),
        }
    }

    #[inline]
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.reg_base + offset) as *const u32) }
    }

    fn write_reg(&self, offset: usize, value: u32) {
        unsafe { write_volatile((self.reg_base + offset) as *mut u32, value) }
    }
}

impl Device for RngDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        if node
            .properties()
            .iter()
            .any(|prop| UNSUPPORTED_PROPS.contains(&prop.name()))
        {
            kprintln!(
                "[ERROR][{}] unsupported properties found in device tree node",
                self.id()
            );
            return Err(DriverInitError::DeviceFailed);
        }

        self.enable().map_err(|_| DriverInitError::DeviceFailed)?;

        register_rng(self.clone()).map_err(|_| DriverInitError::DeviceFailed)?;

        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl RandomNumberGenerator for RngDevice {
    fn name(&self) -> &str {
        self.id()
    }

    fn enable(&self) -> RngResult<()> {
        let _guard = self.lock.lock_irq();

        // enable the RNG hardware and set the warm-up count
        if (self.read_reg(RNG_CTRL_REG_OFFSET) & RNG_RBGEN_BIT) == 0 {
            self.write_reg(RNG_STATUS_REG_OFFSET, RNG_WARMUP_COUNT);
            self.write_reg(RNG_CTRL_REG_OFFSET, RNG_RBGEN_BIT);
        }

        Ok(())
    }

    fn disable(&self) -> RngResult<()> {
        let _guard = self.lock.lock_irq();

        // disable the RNG hardware
        self.write_reg(RNG_CTRL_REG_OFFSET, 0);

        Ok(())
    }

    // use polling to read random data from the RNG hardware (ignore interrupts)
    fn read(&self, buffer: &mut [u8], wait: bool) -> RngResult<usize> {
        let _guard = self.lock.lock_irq();

        let mut retries =
            1_000_000 / (RNG_FIFO_SIZE * RNG_WAIT_PER_UNIT.as_micros() as usize) as isize;
        let mut available_bytes;

        loop {
            let num_units = self.read_reg(RNG_STATUS_REG_OFFSET) >> 24;
            available_bytes = (num_units as usize) * UNIT_SIZE;

            if available_bytes > 0 || !wait || retries <= 0 {
                break;
            }

            retries -= 1;

            busy_wait(RNG_WAIT_PER_UNIT);
        }

        let bytes_to_copy = available_bytes.min(buffer.len());
        (0..bytes_to_copy)
            .into_iter()
            .step_by(UNIT_SIZE)
            .for_each(|i| {
                let data = self.read_reg(RNG_DATA_REG_OFFSET);
                let data_bytes = data.to_le_bytes();

                let chunk_len = (bytes_to_copy - i).min(UNIT_SIZE);
                buffer[i..i + chunk_len].copy_from_slice(&data_bytes[..chunk_len]);
            });

        Ok(bytes_to_copy)
    }
}

pub struct RngDriver {
    dev_registry: DriverRegistry<RngDevice>,
}

impl PlatformDriver for RngDriver {
    fn compatible(&self) -> &[&str] {
        &["brcm,bcm2835-rng"]
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
        kprintln!(
            "{:?} reg: phys_addr=0x{:x}, length=0x{:x}",
            self.compatible(),
            phys_addr,
            length
        );

        let addr = map_io_region(phys_addr, length);
        let dev = RngDevice::new(node.path(), addr);
        let dev = Arc::new(dev);

        dev.clone().global_setup(node)?;

        self.dev_registry.add_device(node.path(), dev);

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

impl RngDriver {
    const fn new() -> Self {
        RngDriver {
            dev_registry: DriverRegistry::new(),
        }
    }
}

pub static DRIVER: RngDriver = RngDriver::new();
