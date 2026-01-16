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

#[derive(Debug)]
pub enum TerminalError {
    NoInputDevice,
    NoOutputDevice,
    AlreadyInitialized,
}

pub type TerminalResult<T> = Result<T, TerminalError>;

pub trait InputDevice: Device {
    fn read(&self) -> Vec<u8>;
}

pub trait OutputDevice: Device {
    fn write(&self, bytes: &[u8]);
}

pub struct SystemTerminal {
    output: Arc<dyn OutputDevice>,
    input: Arc<dyn InputDevice>,
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

pub(super) fn init() -> TerminalResult<()> {
    let output_devs = OUTPUT_DEVICES.lock();
    let input_devs = INPUT_DEVICES.lock();

    if output_devs.is_empty() {
        return Err(TerminalError::NoOutputDevice);
    }

    if input_devs.is_empty() {
        return Err(TerminalError::NoInputDevice);
    }

    let mut system_output: Option<Arc<dyn OutputDevice>> = None;
    let mut system_input: Option<Arc<dyn InputDevice>> = None;

    let dt = get_devicetree().expect("Failed to get devicetree");
    if let Some(chosen) = dt.chosen() {
        if let Some(stdout_path) = chosen.stdout_path() {
            let path = match stdout_path.split_once(':') {
                Some((p, _)) => p,
                None => stdout_path,
            };

            // prioritize stdout-path specified device
            if let Some(stdout_node) = dt.node_by_path(path)
                && let Some(stdout_dev) = output_devs
                    .iter()
                    .find(|dev| dev.id() == stdout_node.path())
            {
                system_output = Some(stdout_dev.clone());

                // check if the same device also supports input
                if let Some(input_dev) =
                    input_devs.iter().find(|dev| dev.id() == stdout_node.path())
                {
                    system_input = Some(input_dev.clone());
                } else if let Some(stdin_path) = chosen.stdin_path() {
                    let path = match stdout_path.split_once(':') {
                        Some((p, _)) => p,
                        None => stdout_path,
                    };

                    // or else check if stdin-path is specified
                    if let Some(stdin_node) = dt.node_by_path(path)
                        && let Some(stdin_dev) =
                            input_devs.iter().find(|dev| dev.id() == stdin_node.path())
                    {
                        system_input = Some(stdin_dev.clone());
                    }
                }
            } else if let Some(stdin_node) = dt.node_by_path(path)
                && let Some(stdin_dev) = input_devs.iter().find(|dev| dev.id() == stdin_node.path())
            {
                system_input = Some(stdin_dev.clone());

                // also check if stdin-path is also an output device
                if let Some(output_dev) =
                    output_devs.iter().find(|dev| dev.id() == stdin_node.path())
                {
                    system_output = Some(output_dev.clone());
                }
            }
        }
    }

    if system_output.is_none() {
        system_output = Some(
            output_devs
                .iter()
                .find(|output_dev| {
                    input_devs
                        .iter()
                        .any(|input_dev| input_dev.id() == output_dev.id())
                })
                .unwrap_or(
                    // SAFETY: we check that output_devs is not empty above
                    output_devs.first().unwrap(),
                )
                .clone(),
        );
    }

    if system_input.is_none() {
        system_input = Some(
            input_devs
                .iter()
                .find(|input_dev|
                    // SAFETY: system_output is guaranteed to be Some here
                    input_dev.id() == system_output.as_ref().unwrap().id())
                .unwrap_or(
                    // SAFETY: we check that output_devs is not empty above
                    input_devs.first().unwrap(),
                )
                .clone(),
        );
    }

    let terminal = SystemTerminal {
        output: system_output.unwrap(),
        input: system_input.unwrap(),
    };

    SYSTEM_TERMINAL
        .set(terminal)
        .map_err(|_| TerminalError::AlreadyInitialized)?;

    Ok(())
}
