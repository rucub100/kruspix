// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::string::String;
use alloc::sync::Arc;

use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::clk::{Clock, ClockResult, register_clock};
use crate::kernel::devicetree::misc_prop::MiscellaneousProperties;
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::mm::map_io_region;

const AUX_IRQ_OFFSET: usize = 0x0;
const AUX_ENABLES_OFFSET: usize = 0x4;

const CLOCK_SPECIFIER_UART: u32 = 0;
const CLOCK_SPECIFIER_SPI1: u32 = 1;
const CLOCK_SPECIFIER_SPI2: u32 = 2;

struct ClockDevice {
    id: String,
    reg_base: usize,
}

impl ClockDevice {
    const fn new(id: String, reg_base: usize) -> Self {
        Self { id, reg_base }
    }
}

impl Device for ClockDevice {
    fn id(&self) -> &str {
        todo!()
    }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        register_clock(node, self).map_err(|_| DriverInitError::DeviceFailed)?;
        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl Clock for ClockDevice {
    fn name(&self) -> &str {
        self.id.as_str()
    }

    fn startup(&self) -> ClockResult<()> {
        Ok(())
    }

    fn shutdown(&self) -> ClockResult<()> {
        Ok(())
    }

    fn enable(&self) -> ClockResult<()> {
        todo!()
    }

    fn disable(&self) -> ClockResult<()> {
        todo!()
    }

    fn get_rate(&self) -> u64 {
        todo!()
    }

    fn set_rate(&self, hz: u64) -> ClockResult<()> {
        todo!()
    }
}

pub struct ClockDriver {
    dev_registry: DriverRegistry<ClockDevice>,
}

impl PlatformDriver for ClockDriver {
    fn compatible(&self) -> &[&str] {
        &["brcm,bcm2835-aux"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        let clock_cells = node.clock_cells().ok_or(DriverInitError::DeviceTreeError)?;
        if clock_cells != 1 {
            return Err(DriverInitError::DeviceTreeError);
        }

        let reg = node.reg().ok_or(DriverInitError::DeviceTreeError)?;
        if reg.len() != 1 {
            return Err(DriverInitError::DeviceTreeError);
        }

        let (phys_addr, length) = node
            .resolve_phys_address_and_length(0)
            .ok_or(DriverInitError::DeviceTreeError)?;
        let addr = map_io_region(phys_addr, length);

        let dev = ClockDevice::new(node.path(), addr);
        let dev = Arc::new(dev);

        dev.clone().global_setup(node)?;

        self.dev_registry.add_device(node.path(), dev.clone());

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

impl ClockDriver {
    const fn new() -> Self {
        Self {
            dev_registry: DriverRegistry::new(),
        }
    }
}

pub static DRIVER: ClockDriver = ClockDriver::new();
