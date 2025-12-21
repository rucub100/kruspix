use alloc::vec::Vec;

use crate::kernel::devicetree::{
    fdt::Fdt, get_devicetree, node::Node, std_prop::StandardProperties,
};
use crate::kprintln;

mod bluetooth;
mod clock_controller;
mod display;
mod dma_controller;
mod ethernet;
mod interrupt_controller;
mod mailbox;
mod mmc;
mod pinctrl;
mod rng;
mod serial;
mod syscon;
mod timer;
mod usb;
mod watchdog;
mod wifi;

#[derive(Debug, Copy, Clone)]
pub enum DriverInitError {
    DeviceReserved,
    DeviceDisabled,
    DeviceFailed,
    Retry,
    DeviceTreeError,
    ToDo,
}

pub trait PlatformDriver {
    /// Returns the compatible string that this driver supports.
    fn compatible(&self) -> &str;

    /// Driver's factory method to initialize a device instance from a device tree node.
    fn try_init(&self, node: &Node) -> Result<(), DriverInitError>;

    /// Optional static initialization method, maybe called during early boot.
    fn early_init(&'static self, _fdt: &Fdt, _path: &str) {
        // default implementation: do nothing
        // can be overridden by specific drivers to support static initialization
        // kernel may not call this method at all
    }
}

pub const PLATFORM_DRIVERS: &[&dyn PlatformDriver] = &[
    // interrupt controllers
    &interrupt_controller::bcm2836_l1_intc::DRIVER,
    &interrupt_controller::bcm2836_armctrl_ic::DRIVER,
    // serial devices
    &serial::bcm2835_aux_uart::DRIVER,
];

pub fn init_platform_drivers() {
    kprintln!("Initializing platform drivers...");

    let dt = get_devicetree().expect("Failed to get devicetree");
    let mut uninitialized_device_nodes: Vec<&Node> = dt
        .root()
        .iter()
        .filter(|node| node.compatible().is_some())
        .collect();

    kprintln!("Match devices from Device Tree:");
    let mut progress = true;
    while progress && !uninitialized_device_nodes.is_empty() {
        progress = false;

        uninitialized_device_nodes.retain(|node| {
            if let Some(driver) = match_driver(node) {
                kprintln!("-> MATCH");
                return match driver.try_init(node) {
                    Ok(_) => {
                        kprintln!("-> Driver initialized successfully");
                        progress = true;
                        false
                    }
                    Err(DriverInitError::Retry) => {
                        kprintln!("-> Driver failed to initialize (RETRY)");
                        true
                    }
                    Err(_) => {
                        kprintln!("-> Driver failed to initialize (SKIP)");
                        false
                    }
                };
            }

            kprintln!("-> No matching driver found");
            false
        });
    }
}

fn match_driver(node: &Node) -> Option<&dyn PlatformDriver> {
    let compatible_list = node.compatible()?;

    kprintln!("Node {} compatible with {:?}", node.path(), compatible_list);
    if let Some(x) = node.phandle() {
        kprintln!("-> has phandle {}", x.0);
    }

    for compatible in compatible_list {
        let driver = PLATFORM_DRIVERS
            .iter()
            .find(|d| d.compatible() == compatible);

        if driver.is_some() {
            return driver.copied();
        }
    }

    None
}
