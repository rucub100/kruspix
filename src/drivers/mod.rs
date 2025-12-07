use crate::kernel::devicetree::fdt::{Fdt, node::Node as FdtNode};
use crate::kernel::devicetree::{Node, PropertyValue, StandardProperty, get_devicetree};
use crate::kprintln;

pub mod mini_uart;
pub mod platform;

pub trait PlatformDriver {
    /// Returns the compatible string that this driver supports.
    fn compatible(&self) -> &str;

    /// Driver's factory method to initialize a device instance from a device tree node.
    fn init(&self, node: &Node);

    /// Optional static initialization method, maybe called during early boot.
    fn static_init(&self, fdt: &Fdt, path: &str) {
        // default implementation: do nothing
        // can be overridden by specific drivers to support static initialization
        // kernel may not call this method at all
    }
}

pub const PLATFORM_DRIVERS: &[&dyn PlatformDriver] = &[&platform::brcm::bcm2835_aux_uart::DRIVER];

pub fn init_platform_drivers() {
    kprintln!("Initializing drivers...");

    // FIXME: split the logic into two phases:
    // 1. discover and store devices in the kernel as device objects
    // 2. initialize drivers for the devices
    let dt = get_devicetree();
    let root = dt.root();
    assert!(dt.version() >= 17);
    assert_eq!(dt.last_compatible_version(), 16);
    assert!(root.is_root());
    assert!(root.name().is_empty());
    assert_eq!(root.path(), "/");

    kprintln!("Match devices from Device Tree:");
    root.iter().for_each(|node| {
        match_driver(&node);
    });
}

fn match_driver(node: &Node) {
    let compatible_prop = node
        .properties()
        .iter()
        .find(|p| p.name() == StandardProperty::COMPATIBLE);

    if let Some(compatible_prop) = compatible_prop {
        if let PropertyValue::Standard(StandardProperty::Compatible(compatible_list)) =
            compatible_prop.value()
        {
            kprintln!("Node {} compatible with {:?}", node.path(), compatible_list);
        } else {
            kprintln!("WARNING: 'compatible' property has unexpected format");
            return;
        }
    }
}
