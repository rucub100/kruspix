use crate::kernel::boot::devicetree::Fdt;
use crate::kernel::boot::devicetree::fdt_structure_block::StructureBlockEntryKind;

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
        // TODO: iterate props if this is memory or reserved-memory node
        let is_mem = node.is_memory();
        count_nodes += 1;
    }

    panic!("TODO");

    // TODO: construct free memory ranges from the /memory node and /reserved-memory nodes
    // as well as the memory reservation block
}
