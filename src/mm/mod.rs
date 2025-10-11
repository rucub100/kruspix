use core::sync::atomic;
use core::sync::atomic::AtomicBool;

pub mod allocator;

static GLOBAL_PHYS_MEM_INIT: AtomicBool = AtomicBool::new(false);
// TODO: create some sync primitives in the kernel and use before application processors are started
static mut GLOBAL_PHYS_MEM: [(usize, usize); 32] = [(0, 0); 32];

pub fn init_phys_mem(mem: &[(usize, usize); 32]) {
    if GLOBAL_PHYS_MEM_INIT.swap(true, atomic::Ordering::SeqCst) {
        panic!("early physical memory already initialized");
    }

    unsafe {
        GLOBAL_PHYS_MEM = *mem;
    }
}
