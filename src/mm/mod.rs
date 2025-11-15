pub mod allocator;

static mut GLOBAL_PHYS_MEM_INIT: bool = false;
static mut GLOBAL_PHYS_MEM: [(usize, usize); 32] = [(0, 0); 32];

/// Initialize early physical memory management.
///
/// ### Safety
/// This function must be called only once during early kernel initialization.
/// Only the primary core must call this function before starting other cores.
#[unsafe(no_mangle)]
pub extern "C" fn init_phys_mem(mem: &[(usize, usize); 32]) {
    unsafe {
        if GLOBAL_PHYS_MEM_INIT {
            panic!("early physical memory already initialized");
        }

        GLOBAL_PHYS_MEM_INIT = true;
        GLOBAL_PHYS_MEM = *mem;
    }
}
