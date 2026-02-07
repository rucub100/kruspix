// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

#![no_std]
#![no_main]
extern crate alloc;

use alloc::format;
use core::time::Duration;

use kruspix::arch::cpu::{local_enable_irq_fiq, wait_for_interrupt};
use kruspix::arch::{kernel::setup::setup_arch, mm::mmu::setup_page_tables};
use kruspix::drivers::init_platform_drivers;
use kruspix::kernel::cpu::init_local_data;
use kruspix::kernel::devicetree::init_devicetree;
use kruspix::kernel::init_modules;
use kruspix::kernel::sched::{add_task, start_sched};
use kruspix::kernel::shell::KernelShell;
use kruspix::kernel::sync::without_irq_fiq;
use kruspix::kernel::terminal::get_system_terminal;
use kruspix::kernel::time::{sleep, uptime};
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
    init_modules();

    add_task("kernel_shell", || {
        KernelShell::new().start();

        loop {
            if let Some(terminal) = get_system_terminal() {
                terminal.poll();
            }

            wait_for_interrupt();
        }
    });

    add_task("show_uptime", || {
        loop {
            let uptime = uptime();
            let secs = uptime.as_secs();

            sleep(Duration::from_millis(500));

            let uptime_str = format!(
                "\x1b[?2026h\x1b[?25l\x1b7\x1b[1;1H\x1b[999C\x1b[23D\x1b[1;33m[ UPTIME: {:>10}s ]\x1b[0m\x1b8\x1b[?25h\x1b[?2026l",
                secs
            );
            if let Some(terminal) = get_system_terminal() {
                without_irq_fiq(|| {
                    terminal.write(uptime_str.as_bytes());
                });
            }
        }
    });

    start_sched()
}
