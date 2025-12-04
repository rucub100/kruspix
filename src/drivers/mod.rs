use crate::kernel::devicetree::{Node, PropertyValue, get_devicetree};
use crate::kprintln;

pub mod mini_uart;
pub mod platform;

pub trait PlatformDriver {
    fn compatible(&self) -> &str;
    fn init(&self, node: &Node);
}

const PLATFORM_DRIVERS: &[&dyn PlatformDriver] = &[&platform::brcm::bcm2835_aux_uart::DRIVER];

pub fn init_platform_drivers() {
    kprintln!("Initializing drivers...");

    let dt = get_devicetree();
    if let Some(dt) = dt {
        let root = dt.root();
        assert!(dt.version() >= 17);
        assert_eq!(dt.last_compatible_version(), 16);
        assert!(root.is_root());
        assert!(root.name().is_empty());
        assert_eq!(root.path(), "/");

        kprintln!("Discover devices from Device Tree:");
        root.iter().for_each(|node| {
            discover_drivers(&node);
        });
    }
}

fn discover_drivers(node: &Node) {
    kprintln!("{}", node.path());
    for prop in node.properties() {
        match prop.value() {
            PropertyValue::Standard(value) => {
                kprintln!("- {}: {:?}", prop.name(), prop.value());
            }
            _ => (),
        };
    }
}
