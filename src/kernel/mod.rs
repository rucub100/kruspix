// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use crate::kprintln;
use crate::mm::virt_to_phys;

pub mod clk;
pub mod console;
pub mod cpu;
pub mod devicetree;
pub mod irq;
pub mod power;
pub mod print;
pub mod rng;
pub mod sched;
pub mod shell;
pub mod sync;
pub mod terminal;
pub mod time;
pub mod watchdog;

/// Initialize all kernel modules
/// 
/// # Safety
/// The order of initialization matters and must be done after the MMU and heap are set up.
pub fn init_modules() {
    match terminal::init() {
        Ok(_) => kprintln!("[INFO] Terminal module initialized successfully"),
        Err(e) => kprintln!("[WARNING] Failed to initialize terminal module: {:?}", e),
    }
    match time::init() {
        Ok(_) => kprintln!("[INFO] Time module initialized successfully"),
        Err(e) => kprintln!("[WARNING] Failed to initialize time module: {:?}", e),
    }
    match rng::init() {
        Ok(_) => kprintln!("[INFO] RNG module initialized successfully"),
        Err(e) => kprintln!("[WARNING] Failed to initialize RNG module: {:?}", e),
    }
    match watchdog::init() {
        Ok(_) => kprintln!("[INFO] Watchdog module initialized successfully"),
        Err(e) => kprintln!("[WARNING] Failed to initialize Watchdog module: {:?}", e),
    }
    match sched::init() {
        Ok(_) => kprintln!("[INFO] Scheduler module initialized successfully"),
        Err(e) => kprintln!("[WARNING] Failed to initialize Scheduler module: {:?}", e),
    }
}

pub fn kernel_addr_size() -> (usize, usize) {
    let kernel_start: usize;
    let kernel_end: usize;

    unsafe {
        // FIXME: this is arch-specific code, move to arch/ module later
        core::arch::asm!("
            ldr {}, =_start
            ldr {}, =_end
        ", out(reg) kernel_start, out(reg) kernel_end);
    }

    (virt_to_phys(kernel_start), kernel_end - kernel_start)
}

pub fn kernel_bss_size() -> usize {
    let bss_start: usize;
    let bss_end: usize;

    unsafe {
        // FIXME: this is arch-specific code, move to arch/ module later
        core::arch::asm!("
            ldr {}, =__bss_start
            ldr {}, =__bss_end
        ", out(reg) bss_start, out(reg) bss_end);
    }

    bss_end - bss_start
}
