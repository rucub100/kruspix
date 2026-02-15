// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::panic::PanicInfo;

use crate::arch::cpu::wait_for_event;
use crate::{kprint, kprintln};

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    kprint!("\n\n\n\n\n\n");
    kprintln!("PANIC OCCURRED:");
    if let Some(location) = _info.location() {
        kprintln!(
            "  File: {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        kprintln!("  No location information available.");
    }

    kprintln!("  Message: {}", _info.message());

    loop {
        wait_for_event();
    }
}
