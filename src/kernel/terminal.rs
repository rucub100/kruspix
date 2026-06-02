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
    inputs: Vec<Arc<dyn InputDevice>>,
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
        for input in self.inputs.iter() {
            let raw_bytes = input.read();
            for byte in raw_bytes {
                if let Some(line) = self.line_discipline(byte) {
                    let listeners = self.listeners.lock();
                    for listener in listeners.iter() {
                        listener.on_line(&line);
                    }
                }
            }
        }
    }

    pub fn add_listener(&self, listener: Arc<dyn LineListener>) {
        self.listeners.lock().push(listener);
    }

    pub fn write(&self, bytes: &[u8]) {
        self.output.write(bytes);
        write_to_secondary_outputs(self.output.id(), bytes);
    }

    fn line_discipline(&self, byte: u8) -> Option<Vec<u8>> {
        let mut state = self.state.lock();

        match byte {
            // handle newline
            b'\n' | b'\r' => {
                if state.echo {
                    self.output.write(&[b'\r', b'\n']);
                    write_to_secondary_outputs(self.output.id(), &[b'\r', b'\n']);
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
                        write_to_secondary_outputs(self.output.id(), b"\x08 \x08");
                    }
                }
                None
            }
            // handle printable characters
            0x20..0x7f => {
                state.line_buffer.push(byte);
                if state.echo {
                    self.output.write(&[byte]);
                    write_to_secondary_outputs(self.output.id(), &[byte]);
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

fn normalize_chosen_path(path: &str) -> &str {
    match path.split_once(':') {
        Some((normalized, _)) => normalized,
        None => path,
    }
}

/// Writes `bytes` to every registered output device whose [`Device::id`] differs
/// from `primary_id`.
///
/// This lets [`SystemTerminal`] mirror all output to secondary devices (e.g.
/// the framebuffer console) without double-writing to the primary UART device.
/// Uses `lock_irq` so it is safe to call from IRQ-disabled contexts.
fn write_to_secondary_outputs(primary_id: &str, bytes: &[u8]) {
    let outputs = OUTPUT_DEVICES.lock_irq();
    for output in outputs.iter() {
        if output.id() != primary_id {
            output.write(bytes);
        }
    }
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

    let dt = get_devicetree().expect("Failed to get devicetree");
    let (stdout_path, stdin_path) = dt
        .chosen()
        .map(|chosen| (chosen.stdout_path(), chosen.stdin_path()))
        .unwrap_or((None, None));

    let find_input_by_id = |id: &str| input_devs.iter().find(|dev| dev.id() == id).cloned();
    let find_output_by_id = |id: &str| output_devs.iter().find(|dev| dev.id() == id).cloned();
    let find_input_for_path = |path: &str| {
        dt.node_by_path(normalize_chosen_path(path))
            .and_then(|node| find_input_by_id(node.path().as_str()))
    };
    let find_output_for_path = |path: &str| {
        dt.node_by_path(normalize_chosen_path(path))
            .and_then(|node| find_output_by_id(node.path().as_str()))
    };
    let find_dedicated_input = || {
        input_devs
            .iter()
            .find(|input_dev| {
                output_devs
                    .iter()
                    .all(|output_dev| output_dev.id() != input_dev.id())
            })
            .cloned()
    };
    let find_dedicated_output = || {
        output_devs
            .iter()
            .find(|output_dev| {
                input_devs
                    .iter()
                    .all(|input_dev| input_dev.id() != output_dev.id())
            })
            .cloned()
    };

    let stdout_output = stdout_path.and_then(find_output_for_path);
    let stdout_input = stdout_path.and_then(find_input_for_path);
    let stdin_output = stdin_path.and_then(find_output_for_path);
    let stdin_input = stdin_path.and_then(find_input_for_path);
    let dedicated_input = find_dedicated_input();
    let dedicated_output = find_dedicated_output();

    let system_input = stdin_input
        .or(dedicated_input)
        .or(stdout_input)
        .or_else(|| input_devs.first().cloned());

    let input_has_matching_output = system_input
        .as_ref()
        .is_some_and(|input_dev| find_output_by_id(input_dev.id()).is_some());

    // Prefer a split terminal (dedicated input + dedicated output) only when the
    // selected input device is not also the selected output device.
    let system_output = if input_has_matching_output {
        stdout_output
            .or_else(|| {
                system_input
                    .as_ref()
                    .and_then(|input_dev| find_output_by_id(input_dev.id()))
            })
            .or(stdin_output)
            .or(dedicated_output)
            .or_else(|| output_devs.first().cloned())
    } else {
        dedicated_output
            .or(stdout_output)
            .or(stdin_output)
            .or_else(|| {
                system_input
                    .as_ref()
                    .and_then(|input_dev| find_output_by_id(input_dev.id()))
            })
            .or_else(|| output_devs.first().cloned())
    };

    let terminal = SystemTerminal {
        output: system_output.unwrap(),
        inputs: input_devs.iter().cloned().collect(),
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
