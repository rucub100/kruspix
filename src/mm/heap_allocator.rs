use core::alloc::Layout;
use core::ptr;

use crate::arch::mm::mmu::{PAGE_SIZE, map_page};
use crate::kernel::boot::sync::BootCell;
use crate::kprintln;

use super::alloc_frame;
use super::allocator::init_global_allocator;
use super::layout::HEAP_MAP_OFFSET;

const MIN_BLOCK_SIZE: usize = 16;
const HEAD_COUNT: usize =
    (PAGE_SIZE.trailing_zeros() - MIN_BLOCK_SIZE.trailing_zeros()) as usize + 1;

#[repr(C)]
struct Node {
    next: *mut Node,
}

#[repr(C)]
struct MultiPageNode {
    size_in_pages: usize,
    next: *mut MultiPageNode,
}

#[repr(C)]
struct Heap {
    heads: [*mut Node; HEAD_COUNT],
    multi_page_list: *mut MultiPageNode,
}

impl Heap {
    const fn new() -> Self {
        Heap {
            heads: [ptr::null_mut(); HEAD_COUNT],
            multi_page_list: ptr::null_mut(),
        }
    }
}

static HEAP_MANAGER: BootCell<Heap> = BootCell::new();
static mut HEAP_SIZE: usize = 0;

#[unsafe(no_mangle)]
pub fn init_heap() {
    kprintln!("[kruspix] Initializing heap...");
    HEAP_MANAGER.init(Heap::new());
    HEAP_MANAGER
        .lock()
        .heads
        .iter_mut()
        .enumerate()
        .for_each(|(index, head_ptr)| {
            refill_head(head_ptr, MIN_BLOCK_SIZE << index);
        });

    init_global_allocator(alloc, dealloc);
}

#[inline(always)]
fn refill_head(head_ptr: &mut *mut Node, block_size: usize) {
    let page = alloc_heap_page();
    for offset in (0..PAGE_SIZE).step_by(block_size) {
        let node_ptr = (page + offset) as *mut Node;
        unsafe { (*node_ptr).next = *head_ptr };
        *head_ptr = node_ptr;
    }
}

#[inline(always)]
fn alloc_heap_page() -> usize {
    let va = unsafe { HEAP_MAP_OFFSET + HEAP_SIZE };
    map_page(va, alloc_frame() as usize);
    unsafe {
        HEAP_SIZE += PAGE_SIZE;
    }

    va
}

unsafe fn alloc(layout: Layout) -> *mut u8 {
    let block_size = layout
        .size()
        .max(layout.align())
        .max(MIN_BLOCK_SIZE)
        .next_power_of_two();
    let head_index = (block_size.trailing_zeros() - MIN_BLOCK_SIZE.trailing_zeros()) as usize;

    if layout.align() > PAGE_SIZE {
        panic!(
            "[kruspix] allocation request too large: size = {}, align = {}",
            layout.size(),
            layout.align()
        );
    }

    let heap = HEAP_MANAGER.lock();
    if head_index >= HEAD_COUNT {
        // TODO: check first if we can reuse any freed multi-page blocks
        let pages = (layout.size() + PAGE_SIZE - 1) / PAGE_SIZE;
        let va = alloc_heap_page();
        // allocate additional pages as needed (contiguous)
        (1..pages).for_each(|_| {
            alloc_heap_page();
        });

        return va as *mut u8;
    }

    let mut head_ptr = &mut heap.heads[head_index];
    if head_ptr.is_null() {
        refill_head(&mut head_ptr, block_size);

        if head_ptr.is_null() {
            return ptr::null_mut();
        }
    }

    let alloc_ptr = *head_ptr;
    *head_ptr = unsafe { (*alloc_ptr).next };

    alloc_ptr as *mut u8
}

unsafe fn dealloc(ptr: *mut u8, layout: Layout) {
    let head_index = (layout
        .size()
        .max(layout.align())
        .max(MIN_BLOCK_SIZE)
        .next_power_of_two()
        .trailing_zeros()
        - MIN_BLOCK_SIZE.trailing_zeros()) as usize;

    if layout.align() > PAGE_SIZE {
        panic!(
            "[kruspix] allocation request too large: size = {}, align = {}",
            layout.size(),
            layout.align()
        );
    }

    let heap = HEAP_MANAGER.lock();

    if head_index >= HEAD_COUNT {
        let multi_page_ptr = ptr as *mut MultiPageNode;
        unsafe {
            (*multi_page_ptr).size_in_pages = (layout.size() + PAGE_SIZE - 1) / PAGE_SIZE;
            (*multi_page_ptr).next = heap.multi_page_list;
            heap.multi_page_list = multi_page_ptr;
        }

        return;
    }

    let head_ptr = &mut heap.heads[head_index];
    let node_ptr = ptr as *mut Node;
    unsafe { (*node_ptr).next = *head_ptr };
    *head_ptr = node_ptr;
}
