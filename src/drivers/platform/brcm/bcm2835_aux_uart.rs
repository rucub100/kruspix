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
                    assert!(node_index > 0);
                    let prop = fdt.parse_standard_prop(node, StandardProp::Reg).unwrap();
                    let (address_cells, size_cells) =
                        fdt.parse_address_and_size_cells(path[node_index - 1].as_ref().unwrap());
                    let (unit_address, _size) = prop
                        .value_as_prop_encoded_array_cells_pair_iter(address_cells, size_cells)
                        .next()
                        .unwrap();
                    unit_address
                });
            let mut reg_base_addr = unit_address;

            // walk up the path and translate the address if necessary (using ranges property)
            for index in (0..node_index).rev() {
                let ancestor = path[index].as_ref().unwrap();
                if let Some(ranges_prop) = fdt.parse_standard_prop(ancestor, StandardProp::Ranges) {
                    let (address_cells, size_cells) = fdt.parse_address_and_size_cells(ancestor);
                    let parent_address_cells = {
                        assert!(index > 0);
                        let parent = path[index - 1].as_ref().unwrap();
                        let (ac, _sc) = fdt.parse_address_and_size_cells(parent);
                        ac
                    };

                    let mut ranges_iter = ranges_prop
                        .value_as_prop_encoded_array_cells_triplet_iter(
                            address_cells,
                            parent_address_cells,
                            size_cells,
                        );
                    let range = ranges_iter.find(|(child_addr, _parent_addr, size)| {
                        reg_base_addr >= *child_addr && reg_base_addr < (*child_addr + *size)
                    });
                    if let Some((child_addr, parent_addr, _size)) = range {
                        reg_base_addr = reg_base_addr - child_addr + parent_addr;
                    }
                }
            }

            STATIC_BASE_ADDR.store(reg_base_addr, Ordering::Release);
        }
    }
}

pub static DRIVER: MiniUart = MiniUart;
