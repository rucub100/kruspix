use crate::kernel::boot::devicetree::Fdt;
use crate::kernel::boot::devicetree::fdt_header::FdtHeader;

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

    // TODO
    for entry in fdt.memory_reservation_block() {
        let addr = entry.address();
        let size = entry.size();
    }


    // TODO: construct free memory ranges from the /memory node and /reserved-memory nodes
    // as well as the memory reservation block
}