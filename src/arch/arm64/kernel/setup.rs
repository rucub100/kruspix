use crate::kernel::boot::devicetree::Fdt;

pub fn setup_arch() {
    let fdt = parse_fdt();
}

pub fn parse_fdt() -> Result<([(usize, usize); 32]), ()> {
    let fdt_addr: usize;

    unsafe {
        core::arch::asm!("mov {}, x0", out(reg) fdt_addr);
    }

    let fdt = Fdt::new(fdt_addr);
    let fdt = fdt.unwrap();

    let aliases = fdt.aliases_node();
    let chosen = fdt.chosen_node();
    let cpus = fdt.cpus_node()?;

    let memory = fdt.parse_memory().unwrap();
    let reserved_memory = fdt.parse_reserved_memory().unwrap();

    // TODO: consider also reserved memory of the kernel image

    Ok((memory))
}
