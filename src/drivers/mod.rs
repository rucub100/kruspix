use crate::kernel::devicetree::get_devicetree;

pub mod mini_uart;

pub fn init_drivers() {
    // TODO: call init for all drivers explicitly
    // the driver may register itself when initialized

    let dt = get_devicetree();
    if let Some(dt) = dt {
        let root = dt.root();
        // TODO: initialize drivers based on device tree nodes
    }
}