// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Result, Write};

use super::sync::SpinLock;
use crate::common::ring_array::RingArray;
use crate::kernel::terminal::OutputDevice;
use crate::kprintln;

pub trait Console: OutputDevice {
    fn write_str(&self, s: &str);
}

impl<T: OutputDevice> Console for T {
    fn write_str(&self, s: &str) {
        self.write(s.as_bytes());
    }
}

/// A buffer to hold early boot messages before the early console is registered
static BOOT_CONSOLE: SpinLock<RingArray<u8, 4096>> = SpinLock::new(RingArray::new(0));
static EARLY_CONSOLE: SpinLock<Option<&'static dyn Console>> = SpinLock::new(None);
static SYSTEM_CONSOLES: SpinLock<Vec<Arc<dyn Console>>> = SpinLock::new(Vec::new());

pub fn register_console(console: Arc<dyn Console>) {
    kprintln!("[INFO] Registering a new system console");

    let mut consoles = SYSTEM_CONSOLES.lock();
    consoles.push(console);
    drop(consoles);

    if let Some(early_console) = EARLY_CONSOLE.lock().take() {
        early_console.write_str("[kruspix] [INFO] Replacing early console with system console\n");
        kprintln!("[INFO] Early console replaced by system console");
    }
}

pub fn register_early_console(console: &'static dyn Console) {
    let mut early_console = EARLY_CONSOLE.lock();

    if early_console.is_none() {
        let mut buf = BOOT_CONSOLE.lock();
        for byte in buf.iter() {
            let slice = core::slice::from_ref(byte);
            if let Ok(msg) = core::str::from_utf8(slice) {
                console.write_str(msg);
            }
        }
        buf.clear();
    }

    *early_console = Some(console);
}

fn console_write_str(s: &str) {
    let consoles = SYSTEM_CONSOLES.lock_irq();
    if !consoles.is_empty() {
        for console in consoles.iter() {
            console.write_str(s);
        }
        return;
    }
    drop(consoles);

    let early_console = EARLY_CONSOLE.lock_irq();
    if let Some(console) = *early_console {
        console.write_str(s);
    } else {
        let mut buf = BOOT_CONSOLE.lock_irq();
        for byte in s.bytes() {
            buf.push(byte);
        }
    }
}

struct ConsoleWriter;

impl Write for ConsoleWriter {
    fn write_str(&mut self, s: &str) -> Result {
        console_write_str(s);
        Ok(())
    }
}

pub fn console_print(args: core::fmt::Arguments) {
    use core::fmt::Write;

    let mut writer = ConsoleWriter;
    let _ = writer.write_fmt(args);
}
