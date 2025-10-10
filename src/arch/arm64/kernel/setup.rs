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

    let memory = fdt.parse_memory_nodes().unwrap();
}
