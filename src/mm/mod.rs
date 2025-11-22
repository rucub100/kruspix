use core::iter;

use crate::kernel::boot::sync::BootCell;
use crate::mm::memory::calc_available_mem;
use crate::{kprint, kprintln};

mod allocator;
mod frame_allocator;
pub mod layout;
mod memory;

pub struct BootPhysMemManager {
    pub available_mem: [(usize, usize); 32],
    pub reserved_mem: [(usize, usize); 32],
    pub kernel_region: (usize, usize),
    pub allocator: frame_allocator::BitMapFrameAllocator,
}

pub static BOOT_PHYS_MEM_MANAGER: BootCell<Option<BootPhysMemManager>> = BootCell::new(None);

#[unsafe(no_mangle)]
pub fn init_phys_mem(
    mem: [(usize, usize); 32],
    reserved_mem: [(usize, usize); 32],
    kernel_region: (usize, usize),
    fdt_addr: usize,
) {
    if BOOT_PHYS_MEM_MANAGER.lock().is_some() {
        panic!("early physical memory already initialized");
    }

    kprintln!("[kruspix] Calculating available physical memory...");
    let available_mem = calc_available_mem(mem, &reserved_mem, kernel_region);

    kprintln!("[kruspix] Available physical memory regions:");
    for (addr, size) in available_mem.iter().filter(|(_, size)| *size > 0) {
        kprintln!("[kruspix] - address: {:#x}, size: {:#x} bytes", addr, size);
    }

    kprintln!("[kruspix] Reserved physical memory regions:");
    for (addr, size) in reserved_mem
        .iter()
        .filter(|(_, size)| *size > 0)
        .chain(iter::once(&kernel_region))
    {
        kprint!("[kruspix] - address: {:#x}, size: {:#x} bytes", addr, size);

        match addr {
            s if s == &kernel_region.0 => kprintln!(" (KERNEL)"),
            s if s == &fdt_addr => kprintln!(" (FDT)"),
            _ => kprintln!(" (I/O PERIPHERALS)"),
        }
    }

    BOOT_PHYS_MEM_MANAGER.lock().replace(BootPhysMemManager {
        available_mem,
        reserved_mem,
        kernel_region,
        allocator: frame_allocator::BitMapFrameAllocator::new(
            available_mem[0].0,
            available_mem[0].1,
            4096,
        ),
    });
}
