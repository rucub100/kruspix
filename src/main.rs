#![no_std]
#![no_main]

use crate::framebuffer::{init_framebuffer, print};
use crate::setup::setup_arch;

mod drivers;
mod kernel;
mod mm;
mod panic_handler;

#[path = "drivers/mailbox/bcm2835_mailbox.rs"]
mod bcm2835_mailbox;

#[cfg(target_arch = "aarch64")]
#[path = "arch/arm64/kernel/entry.rs"]
mod entry;

#[path = "drivers/video/framebuffer.rs"]
mod framebuffer;

#[cfg(target_arch = "aarch64")]
#[path = "arch/arm64/kernel/setup.rs"]
mod setup;

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    kprintln!("\n\n\n\n\n\n[kruspix] Starting kruspix kernel...");
    setup_arch();
    // TODO: memory management setup
    // + physical page allocator
    // + kernel heap allocator
    // + update page tables with proper mappings (advanced FDT parsing with heap)
    // TODO: interrupts/exceptions setup
    // TODO: Scheduler setup
    // TODO: SMP system setup (CPU setup)
    // TODO: Initialize other kernel modules
    // TODO: Initialize device drivers
    // TODO: setup root user space process a.k.a. init
    // TODO: Enable interrupts and start normal operation

    kprintln!("[kruspix] Initialiizing framebuffer...");
    init_framebuffer();

    kprintln!("[kruspix] Testing framebuffer print...");
    print("Hello world!");

    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}
