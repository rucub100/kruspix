// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! TODO: TTY module shall be implemented here
//!
//! Strategy:
//!
//! We need to classify input-only devices (keyboards, mice, touchscreens) and
//! output-only devices (framebuffers, etc.) in order to compose TTYs from them.
//!
//! However, if a device is both input and output (like serial consoles), we shall
//! keep them together in a terminal instance.
//!
//! For the primary console, we shall pick the first available terminal device found
//! or derive it from the stdout-path property in the devicetree (/chosen node).
//!
//! If the primary console happens to be both input and output, we can use it as the "primary terminal".
//! Otherwise, we shall try to compose the terminal from the primary output device and an
//! appropriate input-only device (keyboard, etc.). The input device selection strategy may follow the same
//! principles in this case: check the stdin-path first or fallback to the first input-only device found.
//!
//! Secondary terminals may be created from devices that are not used yes and are both input and output capable
//! or by composing input-only and output-only devices together according to some logic
//! (like matching local keyboards with framebuffers).

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::drivers::Device;
use crate::kernel::devicetree::get_devicetree;
use crate::kernel::devicetree::misc_prop::MiscellaneousProperties;
use crate::kernel::sync::{OnceLock, SpinLock};

pub trait InputDevice: Device {
    fn read(&self) -> Vec<u8>;
}

pub trait OutputDevice: Device {
    fn write(&self, bytes: &[u8]);
}

pub struct SystemTerminal {
    input: Arc<dyn InputDevice>,
    output: Arc<dyn OutputDevice>,
}

static INPUT_DEVICES: SpinLock<Vec<Arc<dyn InputDevice>>> = SpinLock::new(Vec::new());
static OUTPUT_DEVICES: SpinLock<Vec<Arc<dyn OutputDevice>>> = SpinLock::new(Vec::new());

static SYSTEM_TERMINAL: OnceLock<SystemTerminal> = OnceLock::new();

pub fn register_input(dev: Arc<dyn InputDevice>) {
    INPUT_DEVICES.lock().push(dev);
}

pub fn register_output(dev: Arc<dyn OutputDevice>) {
    OUTPUT_DEVICES.lock().push(dev);
}

pub(super) fn init() {
    // TODO:
    // read the devicetree /chosen node for stdout-path and stdin-path properties
    // find the corresponding devices among registered input/output devices
    // set up the primary terminal accordingly

    let dt = get_devicetree().expect("Failed to get devicetree");
    if let Some(chosen) = dt.chosen() {
        
        if let Some(stdout_path) = chosen.stdout_path() {
            // TODO get node by path or alias
        }
    }
}