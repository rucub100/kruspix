// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

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

pub trait LineListener: Send + Sync {
    fn on_line(&self, line: &[u8]);
}

static INPUT_DEVICES: SpinLock<Vec<Arc<dyn InputDevice>>> = SpinLock::new(Vec::new());
static OUTPUT_DEVICES: SpinLock<Vec<Arc<dyn OutputDevice>>> = SpinLock::new(Vec::new());

static SYSTEM_TERMINAL: OnceLock<SystemTerminal> = OnceLock::new();

struct TerminalState {
    line_buffer: Vec<u8>,
    echo: bool,
}

pub struct SystemTerminal {
    output: Arc<dyn OutputDevice>,
    input: Arc<dyn InputDevice>,
    state: SpinLock<TerminalState>,
    listeners: SpinLock<Vec<Arc<dyn LineListener>>>,
}

impl SystemTerminal {
    /// The core method of [`SystemTerminal`], [`SystemTerminal::poll`], attempts to read input data and process it.
    /// This method does not block if the value is not ready. It should be called periodically to drive
    /// the input processing.
    ///
    /// SAFETY: Do NOT call this in ISR context.
    pub fn poll(&self) {
        let raw_bytes = self.input.read();
        for byte in raw_bytes {
            if let Some(line) = self.line_discipline(byte) {
                let listeners = self.listeners.lock();
                for listener in listeners.iter() {
                    listener.on_line(&line);
                }
            }
        }
    }

    pub fn add_listener(&self, listener: Arc<dyn LineListener>) {
        self.listeners.lock().push(listener);
    }

    pub fn write(&self, bytes: &[u8]) {
        self.output.write(bytes);
    }

    fn line_discipline(&self, byte: u8) -> Option<Vec<u8>> {
        let mut state = self.state.lock();

        match byte {
            // handle newline
            b'\n' | b'\r' => {
                if state.echo {
                    self.output.write(&[b'\r', b'\n']);
                }
                let line = state.line_buffer.clone();
                state.line_buffer.clear();
                Some(line)
            }
            // handle backspace/delete
            0x08 | 0x7f => {
                if !state.line_buffer.is_empty() {
                    state.line_buffer.pop();
                    if state.echo {
                        self.output.write(b"\x08 \x08");
                    }
                }
                None
            }
            // handle printable characters
            0x20..0x7f => {
                state.line_buffer.push(byte);
                if state.echo {
                    self.output.write(&[byte]);
                }
                None
            }
            // ignore other bytes
            _ => None,
        }
    }
}

pub fn get_system_terminal() -> Option<&'static SystemTerminal> {
    SYSTEM_TERMINAL.get()
}

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
        state: SpinLock::new(TerminalState {
            line_buffer: Vec::new(),
            echo: true,
        }),
        listeners: SpinLock::new(Vec::new()),
    };

    SYSTEM_TERMINAL
        .set(terminal)
        .map_err(|_| TerminalError::AlreadyInitialized)?;

    Ok(())
}
