#![no_std]
#![no_main]

use framebuffer::Framebuffer;
use crate::bcm2835_wdt::bcm2835_wdt_disable;

mod panic_handler;

#[cfg(target_arch = "aarch64")]
#[path = "arch/arm64/kernel/entry.rs"]
mod entry;

#[path = "drivers/mailbox/bcm2835_mailbox.rs"]
mod bcm2835_mailbox;
#[path = "drivers/video/framebuffer.rs"]
mod framebuffer;
#[path = "drivers/watchdog/bcm2835_wdt.rs"]
mod bcm2835_wdt;

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    // TODO: test new impl on real hardware
    bcm2835_wdt_disable();

    // TODO: Arch-specific setup (device tree, etc)
    // TODO: SMP system setup (CPU setup)
    // TODO: memory management setup
    // TODO: interrupts/exceptions setup
    // TODO: Scheduler setup
    // TODO: Initialize other kernel modules
    // TODO: Initialize device drivers
    // TODO: setup root user space process a.k.a. init
    // TODO: Enable interrupts and start normal operation

    match Framebuffer::new(1024, 768) {
        Ok(fb) => {
            // Define some colors (ARGB format, but Alpha is ignored)
            let white = 0x00FFFFFF;
            let green = 0x0000FF00;

            // Print the message
            fb.print("Hello World?", 100, 100, white);
            fb.print("This is running on bare metal!", 100, 120, green);
        }
        Err(_) => {
            // If framebuffer init fails, we can't print anything.
            // Just halt.
        }
    }

    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}
