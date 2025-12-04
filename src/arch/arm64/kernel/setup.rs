use crate::kernel::boot::devicetree::Fdt;
use crate::kernel::{kernel_addr_size, kernel_bss_size};
use crate::kprintln;
use crate::mm::init_phys_mem;

use crate::arch::mm::mmu::setup_page_tables;
use crate::kernel::devicetree::set_fdt;

/// Architecture-specific setup function for ARM64.
///
/// This function parses the Flattened Device Tree (FDT),
/// and initializes the physical memory.
#[unsafe(no_mangle)]
pub fn setup_arch() {
    let fdt_addr = get_fdt_addr();
    let (kernel_addr, kernel_size) = kernel_addr_size();
    let kernel_bss_size = kernel_bss_size();
    kprintln!(
        "Kernel address: {:#x}, size: {:#x} bytes, BSS size: {:#x} bytes",
        kernel_addr,
        kernel_size,
        kernel_bss_size
    );

    kprintln!("Parsing Flattened Device Tree (FDT)...");
    let (mem, reserved_mem) = parse_fdt(fdt_addr).unwrap();

    kprintln!("Initializing physical memory...");
    init_phys_mem(mem, reserved_mem, (kernel_addr, kernel_size), fdt_addr);

    kprintln!("Setup page tables...");
    unsafe {
        setup_page_tables();
    }
}

#[inline(always)]
fn get_fdt_addr() -> usize {
    let fdt_addr: usize;

    unsafe {
        core::arch::asm!(
        "ldr {tmp}, =__fdt_address",
        "ldr {out}, [{tmp}]",
        tmp = out(reg) _,
        out = out(reg) fdt_addr);
    }

    fdt_addr
}

fn parse_fdt(fdt_addr: usize) -> Result<([(usize, usize); 32], [(usize, usize); 32]), ()> {
    kprintln!("FDT address: {:#x}", fdt_addr);

    let fdt = Fdt::new(fdt_addr)?;

    let memory = fdt.parse_memory()?;
    let reserved_memory = fdt.parse_reserved_memory()?;

    set_fdt(fdt);

    Ok((memory, reserved_memory))
}
