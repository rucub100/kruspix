use crate::kernel::boot::devicetree::fdt_header::FdtHeader;

pub fn setup_arch() {
    parse_fdt();
}

pub fn parse_fdt() {
    let fdt_addr: usize;

    unsafe {
        core::arch::asm!("mov {}, x0", out(reg) fdt_addr);
    }

    let fdt_header = FdtHeader::from_addr(fdt_addr);

    if !fdt_header.is_valid() {
        panic!("FDT header is not valid");
    }

    // TODO: construct free memory ranges from the /memory node and /reserved-memory nodes
    // as well as the memory reservation block
}