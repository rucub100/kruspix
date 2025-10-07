use crate::kernel::boot::devicetree::{Fdt, node::NodeKind};

pub fn setup_arch() {
    parse_fdt();
}

pub fn parse_fdt() {
    let fdt_addr: usize;

    unsafe {
        core::arch::asm!("mov {}, x0", out(reg) fdt_addr);
    }

    let fdt = Fdt::new(fdt_addr);
    let fdt = fdt.unwrap();

    let mut count_mem_resv = 0;
    for entry in fdt.memory_reservation_block_iter() {
        let addr = entry.address();
        let size = entry.size();
        count_mem_resv += 1;
    }

    let mut count_nodes = 0;
    for node in fdt.nodes_iter() {
        match node.kind() {
            NodeKind::Memory => {
                for prop in fdt.props_iter(&node) {
                    let name = prop.name();
                    let value = prop.value();
                }
            }
            _ => { /* ignore other nodes for now */ }
        }
        count_nodes += 1;
    }

    panic!("TODO");

    // TODO: construct free memory ranges from the /memory node and /reserved-memory nodes
    // as well as the memory reservation block
}
