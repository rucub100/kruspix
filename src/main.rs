#![no_std]
#![no_main]

use kruspix::drivers::init_drivers;
use kruspix::kprintln;
use kruspix::mm::init_heap;
use kruspix::setup_arch;

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    kprintln!("\n\n\n\n\n\n[kruspix] Starting kruspix kernel...");
    setup_arch();
    init_heap();
    init_drivers();

    // TODO: memory management setup
    // TODO: interrupts/exceptions setup
    // TODO: Scheduler setup
    // TODO: SMP system setup (CPU setup)
    // TODO: Initialize other kernel modules
    // TODO: Initialize device drivers
    // TODO: setup root user space process a.k.a. init
    // TODO: Enable interrupts and start normal operation

    kprintln!("[kruspix] Kernel initialization complete. Entering idle loop.");
    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}
