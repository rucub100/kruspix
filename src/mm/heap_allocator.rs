use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use super::alloc_frame;
use crate::arch::mm::mmu::{PAGE_SIZE, map_page};
use crate::kprintln;
use crate::mm::layout::HEAP_MAP_OFFSET;

#[repr(C)]
struct Node {
    size: usize,
    next: Option<&'static mut Node>,
}

static HEAP: AtomicPtr<Node> = AtomicPtr::new(ptr::null_mut());
static mut HEAP_SIZE: usize = 0;

#[unsafe(no_mangle)]
pub fn init_heap() {
    kprintln!("[kruspix] Initializing heap...");
    map_page(HEAP_MAP_OFFSET, alloc_frame() as usize);

    let node = unsafe { &mut *(HEAP_MAP_OFFSET as *mut Node) };
    node.size = PAGE_SIZE;
    node.next = None;

    unsafe {
        HEAP_SIZE = PAGE_SIZE;
    }
    
    // TODO initialize the global allocator with the heap alloc and dealloc functions
    
    HEAP.store(node, Ordering::Release);
}
