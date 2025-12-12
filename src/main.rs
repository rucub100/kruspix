#![no_std]
#![no_main]

use kruspix::arch::{kernel::setup::setup_arch, mm::mmu::setup_page_tables};
use kruspix::drivers::init_platform_drivers;
use kruspix::kernel::devicetree::init_devicetree;
use kruspix::mm::init_heap;
use kruspix::{kprint, kprintln};

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    kprint!("\n\n\n\n\n\n");
    kprintln!("Starting kernel initialization...");

    setup_arch();
    setup_page_tables();
    init_heap();
    init_devicetree();
    init_platform_drivers();

    // TODO: memory management setup
    // TODO: interrupts/exceptions setup
    // TODO: Scheduler setup
    // TODO: SMP system setup (CPU setup)
    // TODO: Initialize other kernel modules
    // TODO: Initialize device drivers
    // TODO: setup root user space process a.k.a. init
    // TODO: Enable interrupts and start normal operation

    kprintln!("Kernel initialization complete. Entering idle loop.");
    loop {
        unsafe {
            core::arch::asm!("wfe");
        }
    }
}
