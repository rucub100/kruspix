#![no_std]
#![no_main]

mod panic_handler;

#[cfg(target_arch = "aarch64")]
#[path = "arch/arm64/kernel/entry.rs"]
mod entry;

static HELLO: &[u8] = b"Hello World?";

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    // TODO: abstraction layer for early print
    // TODO: research debugging and println options for qemu
    // TODO: arch_setup() -> parse DTB or EFI System Table (identify by magic signature)
    let uart = 0x0900_0000 as *mut u8;
    for &byte in HELLO {
        unsafe {
            *uart = byte;
            for _ in 0..10000 {
                core::arch::asm!("nop");
            } // crude delay
        }
    }

    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}
