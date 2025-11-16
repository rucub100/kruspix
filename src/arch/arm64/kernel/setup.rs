use crate::kernel::boot::devicetree::Fdt;
use crate::kernel::{kernel_addr_size, kernel_bss_size};
use crate::kprintln;
use crate::mm::{BOOT_PHYS_MEM_MANAGER, PageFrameAllocator, init_phys_mem};

/// Architecture-specific setup function for ARM64.
///
/// This function parses the Flattened Device Tree (FDT),
/// and initializes the physical memory.
#[unsafe(no_mangle)]
pub fn setup_arch() {
    let fdt_addr: usize;

    unsafe {
        core::arch::asm!("mov {}, x0", out(reg) fdt_addr);
    }

    let (kernel_addr, kernel_size) = kernel_addr_size();
    let kernel_bss_size = kernel_bss_size();
    kprintln!(
        "[kruspix] Kernel address: {:#x}, size: {:#x} bytes, BSS size: {:#x} bytes",
        kernel_addr,
        kernel_size,
        kernel_bss_size
    );

    kprintln!("[kruspix] Parsing Flattened Device Tree (FDT)...");
    let (mem, reserved_mem) = parse_fdt(fdt_addr).unwrap();

    kprintln!("[kruspix] Initializing physical memory...");
    init_phys_mem(mem, reserved_mem, (kernel_addr, kernel_size), fdt_addr);
}

fn parse_fdt(fdt_addr: usize) -> Result<([(usize, usize); 32], [(usize, usize); 32]), ()> {
    kprintln!("[kruspix] FDT address: {:#x}", fdt_addr);

    let fdt = Fdt::new(fdt_addr);
    let fdt = fdt.unwrap();

    let aliases = fdt.aliases_node();
    let chosen = fdt.chosen_node();
    let cpus = fdt.cpus_node()?;

    let memory = fdt.parse_memory().unwrap();
    let reserved_memory = fdt.parse_reserved_memory().unwrap();

    Ok((memory, reserved_memory))
}
