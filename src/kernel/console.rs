use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::{Result, Write};

use super::sync::SpinLock;
use crate::common::ring_array::RingArray;
use crate::kprintln;

pub trait Console: Send + Sync {
    fn write(&self, s: &str);
}

/// A buffer to hold early boot messages before the early console is registered
static BOOT_CONSOLE: SpinLock<RingArray<u8, 4096>> = SpinLock::new(RingArray::new(0));
static EARLY_CONSOLE: SpinLock<Option<&'static dyn Console>> = SpinLock::new(None);
static SYSTEM_CONSOLES: SpinLock<Vec<Box<dyn Console>>> = SpinLock::new(Vec::new());

pub fn register_console(console: Box<dyn Console>) {
    let mut consoles = SYSTEM_CONSOLES.lock();

    kprintln!("INFO: registering a new system console");

    consoles.push(console);
}

pub fn register_early_console(console: &'static dyn Console) {
    let mut early_console = EARLY_CONSOLE.lock();

    if early_console.is_some() {
        kprintln!("WARNING: early console already registered, overwriting...");
    } else {
        let mut buf = BOOT_CONSOLE.lock();
        for byte in buf.iter() {
            let slice = core::slice::from_ref(byte);
            if let Ok(msg) = core::str::from_utf8(slice) {
                console.write(msg);
            }
        }
        buf.clear();
    }

    *early_console = Some(console);
}

fn console_write_str(s: &str) {
    let consoles = SYSTEM_CONSOLES.lock();
    if !consoles.is_empty() {
        return;
    }
    drop(consoles);

    let early_console = EARLY_CONSOLE.lock();
    if let Some(console) = *early_console {
        console.write(s);
    } else {
        let mut buf = BOOT_CONSOLE.lock();
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
