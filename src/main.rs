#![no_std]
#![no_main]

use framebuffer::Framebuffer;

mod panic_handler;

#[cfg(target_arch = "aarch64")]
#[path = "arch/arm64/kernel/entry.rs"]
mod entry;

mod mailbox;
mod framebuffer;

static HELLO: &[u8] = b"Hello World?";

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    // TODO: abstraction layer for early print
    // TODO: research debugging and println options for qemu
    // TODO: arch_setup() -> parse DTB or EFI System Table (identify by magic signature)

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
