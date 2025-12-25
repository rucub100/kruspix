#![no_std]
#![no_main]
extern crate alloc;

use alloc::sync::Arc;
use core::time::Duration;
use kruspix::arch::cpu::enable_irq_fiq;
use kruspix::arch::{kernel::setup::setup_arch, mm::mmu::setup_page_tables};
use kruspix::drivers::init_platform_drivers;
use kruspix::kernel::devicetree::init_devicetree;
use kruspix::kernel::time::{cancel_alarm, test_alarm};
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

    enable_irq_fiq();

    test_alarm(
        Duration::from_secs(2),
        Arc::new(|_| {
            kprintln!("Alarm triggered after 2 seconds!");
            cancel_alarm();
        }),
    );

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
