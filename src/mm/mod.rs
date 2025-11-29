use core::iter;
use core::ptr;

use crate::arch::mm::mmu::PAGE_SIZE;
use crate::kernel::boot::sync::BootCell;
use crate::{kprint, kprintln};

use frame_allocator::{BitMapFrameAllocator, PageFrameAllocator};
use layout::LINEAR_MAP_OFFSET;
use memory::calc_available_mem;

mod allocator;
mod frame_allocator;
mod heap_allocator;
pub mod layout;
mod memory;

pub struct BootPhysMemManager {
    pub available_mem: [(usize, usize); 32],
    pub reserved_mem: [(usize, usize); 32],
    pub kernel_region: (usize, usize),
    pub allocator: BitMapFrameAllocator,
}

static BOOT_PHYS_MEM_MANAGER: BootCell<BootPhysMemManager> = BootCell::new();

#[unsafe(no_mangle)]
pub fn init_phys_mem(
    mem: [(usize, usize); 32],
    reserved_mem: [(usize, usize); 32],
    kernel_region: (usize, usize),
    fdt_addr: usize,
) {
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

    BOOT_PHYS_MEM_MANAGER.init(BootPhysMemManager {
        available_mem,
        reserved_mem,
        kernel_region,
        allocator: BitMapFrameAllocator::new(available_mem[0].0, available_mem[0].1, PAGE_SIZE),
    });
}

#[inline]
pub const fn virt_to_phys(va: usize) -> usize {
    if va >= LINEAR_MAP_OFFSET {
        va - LINEAR_MAP_OFFSET
    } else {
        va
    }
}

#[inline]
pub const fn phys_to_virt(pa: usize) -> usize {
    pa + LINEAR_MAP_OFFSET
}

#[inline]
pub fn alloc_frame() -> *mut u8 {
    unsafe { BOOT_PHYS_MEM_MANAGER.lock().allocator.alloc_frame() }
}

#[inline]
pub fn dealloc_frame(ptr: *mut u8) {
    unsafe { BOOT_PHYS_MEM_MANAGER.lock().allocator.dealloc_frame(ptr) }
}

/// Allocates a physical frame and returns its linearly mapped virtual address.
///
/// Returns `ptr::null_mut()` if the allocation fails.
///
/// # Safety
/// The returned memory is uninitialized.
#[inline]
pub fn alloc_page() -> *mut u8 {
    unsafe {
        let frame_ptr = alloc_frame();

        if frame_ptr.is_null() {
            return ptr::null_mut();
        }

        phys_to_virt(frame_ptr as usize) as *mut u8
    }
}

#[inline]
pub fn dealloc_page(ptr: *mut u8) {
    unsafe {
        dealloc_frame(virt_to_phys(ptr as usize) as *mut u8);
    }
}
