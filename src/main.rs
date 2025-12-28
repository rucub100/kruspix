#![no_std]
#![no_main]
extern crate alloc;

use alloc::sync::Arc;
use kruspix::arch::cpu::local_enable_irq_fiq;
use kruspix::arch::{kernel::setup::setup_arch, mm::mmu::setup_page_tables};
use kruspix::drivers::init_platform_drivers;
use kruspix::kernel::cpu::{get_local_data, init_local_data};
use kruspix::kernel::devicetree::init_devicetree;
use kruspix::kernel::irq::register_handler;
use kruspix::kernel::time::uptime;
use kruspix::mm::init_heap;
use kruspix::{kprint, kprintln};

#[unsafe(no_mangle)]
pub extern "C" fn start_kernel() -> ! {
    kprint!("\n\n\n\n\n\n");
    kprintln!("Starting kernel initialization...");

    setup_arch();
    setup_page_tables();
    init_heap();
    init_local_data();
    init_devicetree();
    init_platform_drivers();

    local_enable_irq_fiq();

    let local = get_local_data();
    let alarm_handler = Arc::new(|_| {
        let alarm = local.get_alarm().unwrap();
        let now = uptime();
        kprintln!("[{}] Local alarm triggered on core {}", now.as_millis(),  local.core_id());
        alarm.cancel();
        let mut ticks = alarm.duration_to_ticks(now);
        ticks += alarm.duration_to_ticks(core::time::Duration::from_secs(1));
        alarm.schedule_at(ticks);
    });

    if let Some(alarm) = local.get_alarm() {
        register_handler(alarm.virq(), alarm_handler).unwrap();
        let uptime = uptime();
        let mut ticks = alarm.duration_to_ticks(uptime);
        ticks += alarm.duration_to_ticks(core::time::Duration::from_secs(1));
        alarm.schedule_at(ticks);
    }

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
