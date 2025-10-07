use crate::kernel::boot::devicetree::Fdt;

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

    // TODO: extract required properties from this nodes and debug and test values
    let root = fdt.root_node();
    let aliases = fdt.aliases_node();
    let reserved_memory = fdt.reserved_memory_node();
    let chosen = fdt.chosen_node();
    let cpus = fdt.cpus_node();

    for mem_node in fdt.memory_node_iter() {
        // TODO add memory range to an array (fixed size e.g. 32)
    }

    if let Some(reserved_memory_node) = reserved_memory {
        // TODO subtract reserved memory ranges from array
    }

    for entry in fdt.memory_reservation_block_iter() {
        // TODO subtract from memory ranges
    }
}
