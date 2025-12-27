use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::kernel::devicetree::std_prop::StatusValue;
use crate::kernel::devicetree::{
    fdt::Fdt, get_devicetree, node::Node, std_prop::StandardProperties,
};
use crate::kernel::sync::SpinLock;
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

pub trait Device {
    fn global_setup(&self);
    fn local_setup(&self);
    fn path(&self) -> &str;
}

pub trait PlatformDriver {
    /// Returns the compatible string that this driver supports.
    fn compatible(&self) -> &str;

    /// Driver's factory method to initialize a device instance from a device tree node.
    fn try_init(&self, node: &Node) -> Result<(), DriverInitError>;

    fn get_device(&self, node: &Node) -> Option<Arc<dyn Device>>;

    /// Optional per-core local initialization method, maybe called during core boot.
    fn local_init(&self, node: &Node) {
        if let Some(device) = self.get_device(node) {
            device.local_setup();
        }
    }

    /// Optional static initialization method, maybe called during early boot.
    fn early_init(&'static self, _fdt: &Fdt, _path: &str) {
        // default implementation: do nothing
        // can be overridden by specific drivers to support static initialization
        // kernel may not call this method at all
    }
}

pub struct DriverRegistry<T>
where
    T: Device,
{
    devices: SpinLock<BTreeMap<String, Arc<T>>>,
}

impl<T> DriverRegistry<T>
where
    T: Device,
{
    pub const fn new() -> Self {
        DriverRegistry {
            devices: SpinLock::new(BTreeMap::new()),
        }
    }

    pub fn add_device(&self, node: &Node, device: Arc<T>) {
        let mut devices = self.devices.lock_irq();
        devices.insert(node.path(), device);
    }

    pub fn remove_device(&self, node: &Node) -> Option<Arc<T>> {
        let mut devices = self.devices.lock_irq();
        devices.remove(&node.path())
    }

    pub fn get_device(&self, node: &Node) -> Option<Arc<T>> {
        let devices = self.devices.lock_irq();
        devices.get(&node.path()).cloned()
    }
}

impl<T: Device + 'static> DriverRegistry<T> {
    pub fn get_device_opaque(&self, node: &Node) -> Option<Arc<dyn Device>> {
        let devices = self.devices.lock_irq();
        devices
            .get(&node.path())
            .cloned()
            .map(|dev| dev as Arc<dyn Device>)
    }
}

pub const PLATFORM_DRIVERS: &[&dyn PlatformDriver] = &[
    // interrupt controller
    &interrupt_controller::bcm2836_l1_intc::DRIVER,
    &interrupt_controller::bcm2836_armctrl_ic::DRIVER,
    // timer
    &timer::bcm2835_system_timer::DRIVER,
    // serial
    &serial::bcm2835_aux_uart::DRIVER,
];

pub fn init_platform_drivers() {
    kprintln!("Initializing platform drivers...");

    let dt = get_devicetree().expect("Failed to get devicetree");
    let mut uninitialized_device_nodes: Vec<&Node> = dt
        .root()
        .iter()
        .filter(|node| node.compatible().is_some())
        .filter(|node| {
            node.status().is_none_or(|status_value| {
                let compatible_list = node.compatible().unwrap();

                match status_value {
                    StatusValue::Okay => true,
                    StatusValue::Reserved => {
                        kprintln!(
                            "Device {} compatible with {:?} is reserved",
                            node.path(),
                            compatible_list
                        );
                        false
                    }
                    StatusValue::Disabled => {
                        kprintln!(
                            "Device {} compatible with {:?} is disabled",
                            node.path(),
                            compatible_list
                        );
                        false
                    }
                    StatusValue::Fail(fail) => {
                        kprintln!(
                            "Device {} compatible with {:?} has failed status {}",
                            node.path(),
                            compatible_list,
                            fail
                        );
                        false
                    }
                }
            })
        })
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
