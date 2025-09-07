#![no_std]
#![no_main]

use core::ptr::{read_volatile, write_volatile};
use framebuffer::Framebuffer;

mod panic_handler;

#[cfg(target_arch = "aarch64")]
#[path = "arch/arm64/kernel/entry.rs"]
mod entry;

mod mailbox;
mod framebuffer;

static HELLO: &[u8] = b"Hello World?";
// BCM2837 Watchdog Timer registers
const PM_WDOG_BASE: u32 = 0x3F100000;
const PM_RSTC: *mut u32 = (PM_WDOG_BASE + 0x1c) as *mut u32;
const PM_WDOG: *mut u32 = (PM_WDOG_BASE + 0x24) as *mut u32;

const PM_PASSWORD: u32 = 0x5a000000;

fn disable_watchdog() {
    unsafe {
        // Read the reset status register
        let rstc = read_volatile(PM_RSTC);
        // Clear the watchdog reset flag
        write_volatile(PM_RSTC, PM_PASSWORD | (rstc & !0x00000020));
        // Write the timeout to 0
        write_volatile(PM_WDOG, PM_PASSWORD | 0);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    // TODO: abstraction layer for early print
    // TODO: research debugging and println options for qemu
    // TODO: arch_setup() -> parse DTB or EFI System Table (identify by magic signature)
    // in the arch setup: more generic solution for video output is the DTB parsing, it should contain
    // the framebuffer address and resolution, so we don't need to hardcode it

    disable_watchdog();

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
