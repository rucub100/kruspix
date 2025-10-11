use crate::kernel::boot::devicetree::Fdt;
use crate::mm::init_phys_mem;

pub fn setup_arch() {
    let (mem, reserved_mem) = parse_fdt().unwrap();
    let (kernel_addr, kernel_size) = kernel_addr_size();
    let mem = calc_available_mem(mem, &reserved_mem, (kernel_addr, kernel_size));

    init_phys_mem(&mem);
}

fn parse_fdt() -> Result<([(usize, usize); 32], [(usize, usize); 32]), ()> {
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

    Ok((memory, reserved_memory))
}

fn kernel_addr_size() -> (usize, usize) {
    let kernel_start: usize;
    let kernel_end: usize;

    unsafe {
        core::arch::asm!("
            ldr {}, =_start
            ldr {}, =_end
        ", out(reg) kernel_start, out(reg) kernel_end);
    }

    (kernel_start, kernel_end - kernel_start)
}

fn calc_available_mem( mem: [(usize, usize); 32], reserved_mem: &[(usize, usize); 32], kernel_region: (usize, usize)) -> [(usize, usize); 32] {
    let next_mem_index = mem.iter().position(|(addr, size)| *size == 0).unwrap_or(32);

    // TODO subtract kernel region from mem
    // TODO subtract reserved_mem from mem

    mem
}