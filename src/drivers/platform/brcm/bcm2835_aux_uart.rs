use core::sync::atomic::{AtomicUsize, Ordering};

use crate::drivers::PlatformDriver;
use crate::kernel::devicetree::Node;
use crate::kernel::devicetree::fdt::Fdt;
use crate::kernel::devicetree::fdt::prop::StandardProp;

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

static STATIC_BASE_ADDR: AtomicUsize = AtomicUsize::new(0);

pub struct MiniUart;

impl MiniUart {}

impl PlatformDriver for MiniUart {
    fn compatible(&self) -> &str {
        "brcm,bcm2835-aux-uart"
    }

    fn init(&self, node: &Node) {
        // TODO:
        // 1. initialize the hardware
        // 2. we may have to map the MMIO region from the device tree node
        // 3. register the UART as a console device (capability)
        // FIXME: the driver has to be a factory for multiple instances of the device
        // so a global static variable is a bad idea
        // 1 driver <-> N devices
    }

    fn static_init(&self, fdt: &Fdt, path: &str) {
        // TODO: this code is not driver specific, move it to a common utility function
        // which shall translate the reg property of a node into addresses as seen by the CPU
        if let Some(path) = fdt.get_nodes_path(path) {
            let node_index = path.iter().rposition(|x| x.is_some()).unwrap();
            let node = path[node_index].as_ref().unwrap();
            let unit_address = node
                .unit_address()
                .and_then(|s| usize::from_str_radix(s, 16).ok())
                .unwrap_or_else(|| {
                    let prop = fdt.parse_standard_prop(node, StandardProp::Reg).unwrap();
                    let (address_cells, size_cells) = fdt
                        .parse_address_and_size_cells(path[node_index - 1].as_ref().unwrap())
                        .unwrap();
                    let (unit_address, _size) = prop
                        .value_as_prop_encoded_array_cells_pair_iter(address_cells, size_cells)
                        .next()
                        .unwrap();
                    unit_address
                });
            let mut aux_base = unit_address;
            // TODO: walk up the path and translate the address if necessary (using ranges property)
            STATIC_BASE_ADDR.store(aux_base, Ordering::Release);
        }
    }
}

pub static DRIVER: MiniUart = MiniUart;
