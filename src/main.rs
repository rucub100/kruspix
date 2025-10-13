#![no_std]
#![no_main]

use crate::bcm2835_wdt::bcm2835_wdt_disable;
use crate::framebuffer::{init_framebuffer, print};
use crate::setup::setup_arch;

mod panic_handler;
mod kernel;
mod mm;

#[cfg(target_arch = "aarch64")]
#[path = "arch/arm64/kernel/entry.rs"]
mod entry;
#[cfg(target_arch = "aarch64")]
#[path = "arch/arm64/kernel/setup.rs"]
mod setup;
#[path = "drivers/mailbox/bcm2835_mailbox.rs"]
mod bcm2835_mailbox;
#[path = "drivers/watchdog/bcm2835_wdt.rs"]
mod bcm2835_wdt;
#[path = "drivers/video/framebuffer.rs"]
mod framebuffer;

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    // TODO: Arch-specific setup (device tree, etc)
    setup_arch();
    // TODO: memory management setup
    // + setup initial page tables
    // + enable MMU and virtual memory
    // + page allocator
    // + kernel heap allocator
    // TODO: interrupts/exceptions setup
    // TODO: Scheduler setup
    // TODO: SMP system setup (CPU setup)
    // TODO: Initialize other kernel modules
    // TODO: Initialize device drivers
    // TODO: setup root user space process a.k.a. init
    // TODO: Enable interrupts and start normal operation

    init_framebuffer();

    bcm2835_wdt_disable();

    print("Hello world!");

    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}
