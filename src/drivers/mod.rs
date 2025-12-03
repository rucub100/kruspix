use alloc::vec::Vec;

use crate::kernel::boot::sync::BootCell;
use crate::kernel::devicetree::{Node, PropertyValue, get_devicetree};
use crate::kprintln;

pub mod mini_uart;

static DRIVERS: BootCell<Vec<(&'static str, fn())>> = BootCell::new();

pub fn init_drivers() {
    kprintln!("[kruspix] Initializing drivers...");
    DRIVERS.init(Vec::new());

    register_drivers();
    init_compatible_drivers();
}

fn register_drivers() {
    DRIVERS.lock().push(("test", || {}))
}

fn init_compatible_drivers() {
    let dt = get_devicetree();
    if let Some(dt) = dt {
        let root = dt.root();
        assert!(dt.version() >= 17);
        assert_eq!(dt.last_compatible_version(), 16);
        assert!(root.is_root());
        assert!(root.name().is_empty());
        assert_eq!(root.path(), "/");

        kprintln!("[kruspix] Discover devices from Device Tree:");
        root.iter().for_each(|node| {
            discover_drivers(&node);
        });
    }
}

fn discover_drivers(node: &Node) {
    kprintln!("[kruspix] {}", node.path());
    for prop in node.properties() {
        match prop.value() {
            PropertyValue::Standard(value) => {
                kprintln!("[kruspix] - {}: {:?}", prop.name(), prop.value());
            }
            _ => (),
        };
    }
}
