// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::string::String;
use alloc::sync::Arc;

use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::clk::{Clock, ClockError, ClockResult, register_clock};
use crate::kernel::devicetree::misc_prop::MiscellaneousProperty;
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::prop::PropertyValue;

struct FixedClockDevice {
    id: String,
    frequency_hz: u64,
}

impl FixedClockDevice {
    fn new(id: String, frequency_hz: u64) -> Self {
        Self { id, frequency_hz }
    }
}

impl Device for FixedClockDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        register_clock(node, self).map_err(|_| DriverInitError::DeviceFailed)?;
        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl Clock for FixedClockDevice {
    fn name(&self) -> &str {
        self.id()
    }

    fn startup(&self) -> ClockResult<()> {
        Ok(())
    }

    fn shutdown(&self) -> ClockResult<()> {
        Ok(())
    }

    fn enable(&self) -> ClockResult<()> {
        Ok(())
    }

    fn disable(&self) -> ClockResult<()> {
        Ok(())
    }

    fn get_rate(&self) -> u64 {
        self.frequency_hz
    }

    fn set_rate(&self, hz: u64) -> ClockResult<()> {
        match hz {
            _ if hz == self.frequency_hz => Ok(()),
            _ => Err(ClockError::RateNotSupported),
        }
    }
}

pub struct FixedClockDriver {
    dev_registry: DriverRegistry<FixedClockDevice>,
}

impl PlatformDriver for FixedClockDriver {
    fn compatible(&self) -> &[&str] {
        &["fixed-clock"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        let clock_cells = node
            .properties()
            .iter()
            .find(|prop| prop.name() == "#clock-cells")
            .ok_or(DriverInitError::DeviceTreeError)?;

        if match clock_cells.value() {
            PropertyValue::Unknown(prop) => match prop.try_into() {
                Ok(0u32) => false,
                _ => true,
            },
            _ => true,
        } {
            return Err(DriverInitError::DeviceTreeError);
        }

        let clock_frequency_prop = node
            .properties()
            .iter()
            .find(|prop| prop.name() == "clock-frequency")
            .ok_or(DriverInitError::DeviceTreeError)?;

        let frequency_hz = match clock_frequency_prop.value() {
            PropertyValue::Miscellaneous(MiscellaneousProperty::ClockFrequency(freq)) => {
                freq.as_u64()
            }
            _ => return Err(DriverInitError::DeviceFailed),
        };

        let dev = FixedClockDevice::new(node.path(), frequency_hz);
        let dev = Arc::new(dev);

        dev.clone().global_setup(node)?;

        self.dev_registry.add_device(node.path(), dev.clone());

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

impl FixedClockDriver {
    const fn new() -> Self {
        FixedClockDriver {
            dev_registry: DriverRegistry::new(),
        }
    }
}

pub static DRIVER: FixedClockDriver = FixedClockDriver::new();
