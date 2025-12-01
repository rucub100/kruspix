#![no_std]
#![no_main]

use kruspix::init_drivers;
use kruspix::init_heap;
use kruspix::kprintln;
use kruspix::setup_arch;

#[path = "drivers/mailbox/bcm2835_mailbox.rs"]
mod bcm2835_mailbox;

#[path = "drivers/video/framebuffer.rs"]
mod framebuffer;

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    kprintln!("\n\n\n\n\n\n[kruspix] Starting kruspix kernel...");
    setup_arch();
    init_heap();
    init_drivers();
    // TODO: memory management setup
    // + update page tables with proper mappings (advanced FDT parsing with heap)
    // TODO: interrupts/exceptions setup
    // TODO: Scheduler setup
    // TODO: SMP system setup (CPU setup)
    // TODO: Initialize other kernel modules
    // TODO: Initialize device drivers
    // TODO: setup root user space process a.k.a. init
    // TODO: Enable interrupts and start normal operation

    use crate::framebuffer::{init_framebuffer, print};
    kprintln!("[kruspix] Initializing framebuffer...");
    init_framebuffer();

    kprintln!("[kruspix] Testing framebuffer print...");
    print("Hello world!");

    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}
