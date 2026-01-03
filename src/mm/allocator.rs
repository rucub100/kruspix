// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::alloc::{GlobalAlloc, Layout};
use core::mem;
use core::sync::atomic::{AtomicPtr, Ordering};

pub type AllocFn = unsafe fn(layout: Layout) -> *mut u8;
pub type DeallocFn = unsafe fn(ptr: *mut u8, layout: Layout);

struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let alloc_fn_ptr = ALLOC_FN.load(Ordering::Relaxed);
        unsafe {
            let alloc_fn = mem::transmute::<*mut (), AllocFn>(alloc_fn_ptr);
            alloc_fn(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let dealloc_fn_ptr = DEALLOC_FN.load(Ordering::Relaxed);
        unsafe {
            let dealloc_fn = mem::transmute::<*mut (), DeallocFn>(dealloc_fn_ptr);
            dealloc_fn(ptr, layout)
        }
    }
}

unsafe fn _panic_alloc_impl(_layout: Layout) -> *mut u8 {
    panic!("allocation attempted before allocator is initialized");
}

unsafe fn _panic_dealloc_impl(_ptr: *mut u8, _layout: Layout) {
    panic!("deallocation attempted before allocator is initialized");
}

static ALLOC_FN: AtomicPtr<()> = AtomicPtr::new(_panic_alloc_impl as *mut ());
static DEALLOC_FN: AtomicPtr<()> = AtomicPtr::new(_panic_dealloc_impl as *mut ());

#[global_allocator]
static GLOBAL_KERNEL_ALLOCATOR: KernelAllocator = KernelAllocator;

pub fn init_global_allocator(alloc: AllocFn, dealloc: DeallocFn) {
    ALLOC_FN.store(alloc as *mut (), Ordering::Relaxed);
    DEALLOC_FN.store(dealloc as *mut (), Ordering::Relaxed);
}
