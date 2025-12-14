use core::iter;
use core::ptr;

use crate::arch::mm::mmu::{PAGE_SIZE, map_page};
use crate::kprintln;

use crate::kernel::sync::SpinLock;
use crate::mm::layout::{IO_PERIPHERALS_MAP_OFFSET, IO_PERIPHERALS_MAP_SIZE};
use frame_allocator::{BitMapFrameAllocator, PageFrameAllocator};
pub use heap_allocator::init_heap;
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

static BOOT_PHYS_MEM_MANAGER: SpinLock<Option<BootPhysMemManager>> = SpinLock::new(None);

#[unsafe(no_mangle)]
pub fn init_phys_mem(
    mem: [(usize, usize); 32],
    reserved_mem: [(usize, usize); 32],
    kernel_region: (usize, usize),
    fdt_addr: usize,
) {
    kprintln!("Calculating available physical memory...");
    let available_mem = calc_available_mem(mem, &reserved_mem, kernel_region);

    kprintln!("Available physical memory regions:");
    for (addr, size) in available_mem.iter().filter(|(_, size)| *size > 0) {
        kprintln!("-> address: {:#x}, size: {:#x} bytes", addr, size);
    }

    kprintln!("Reserved physical memory regions:");
    for (addr, size) in reserved_mem
        .iter()
        .filter(|(_, size)| *size > 0)
        .chain(iter::once(&kernel_region))
    {
        let suffix = match (addr, size) {
            (addr, _size) if addr == &kernel_region.0 => "(KERNEL)",
            (addr, _size) if addr == &fdt_addr => "(FDT)",
            _ => "(I/O PERIPHERALS)",
        };

        kprintln!(
            "-> address: {:#x}, size: {:#x} bytes {}",
            addr,
            size,
            suffix
        );
    }

    BOOT_PHYS_MEM_MANAGER.lock().replace(BootPhysMemManager {
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
    unsafe {
        BOOT_PHYS_MEM_MANAGER
            .lock()
            .as_mut()
            .unwrap()
            .allocator
            .alloc_frame()
    }
}

#[inline]
pub fn dealloc_frame(ptr: *mut u8) {
    unsafe {
        BOOT_PHYS_MEM_MANAGER
            .lock()
            .as_mut()
            .unwrap()
            .allocator
            .dealloc_frame(ptr)
    }
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

pub fn map_io_region(pa: usize, size: usize) -> usize {
    assert!(
        IO_PERIPHERALS_MAP_OFFSET + pa + size
            <= IO_PERIPHERALS_MAP_OFFSET + IO_PERIPHERALS_MAP_SIZE
    );

    let va = IO_PERIPHERALS_MAP_OFFSET + pa;

    for offset in (0..size).step_by(PAGE_SIZE) {
        map_page(va + offset, pa + offset);
    }

    va
}
