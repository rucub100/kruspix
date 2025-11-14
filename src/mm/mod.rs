// use core::sync::atomic;
// use core::sync::atomic::AtomicBool;

pub mod allocator;

// static GLOBAL_PHYS_MEM_INIT: AtomicBool = AtomicBool::new(false);
static mut GLOBAL_PHYS_MEM_INIT: bool = false;
static mut GLOBAL_PHYS_MEM: [(usize, usize); 32] = [(0, 0); 32];

/// Initialize early physical memory management.
///
/// ### Safety
/// This function must be called only once during early kernel initialization.
/// Only the primary core must call this function before starting other cores.
#[unsafe(no_mangle)]
pub extern "C" fn init_phys_mem(mem: &[(usize, usize); 32]) {
    // FIXME: try to fix exception caused by AtomicBool (ldaxrb instruction)
    // -> this instruction seems to cause an exception on RPi 3b
    // -> one possible cause is an issue in the early MMU configuration
    // -> we can try to add some early exception handling to catch the
    //    sync exception or abort caused by this instruction
    // -> analyze the ESR_EL1 and FAR_EL1 registers to find the root cause
    // --------------------------------------------------------------------------------
    // if GLOBAL_PHYS_MEM_INIT.swap(true, atomic::Ordering::SeqCst) {
    //     panic!("early physical memory already initialized");
    // }

    unsafe {
        if GLOBAL_PHYS_MEM_INIT {
            panic!("early physical memory already initialized");
        }

        GLOBAL_PHYS_MEM_INIT = true;
        GLOBAL_PHYS_MEM = *mem;
    }
}
