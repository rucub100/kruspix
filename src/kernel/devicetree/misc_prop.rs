// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::string::String;

pub const CLOCK_CELLS: &'static str = "#clock-cells";
pub const CLOCK_FREQUENCY: &'static str = "clock-frequency";
pub const BOOT_ARGS: &'static str = "bootargs";
pub const STDOUT_PATH: &'static str = "stdout-path";
pub const STDIN_PATH: &'static str = "stdin-path";

#[derive(Debug)]
pub enum MiscPropError {}

pub type MiscPropResult<T> = Result<T, MiscPropError>;

pub trait MiscellaneousProperties {
    fn clock_cells(&self) -> Option<u32>;
    fn clock_frequency(&self) -> Option<&ClockFrequency>;
    fn boot_args(&self) -> Option<&str>;
    fn stdout_path(&self) -> Option<&str>;
    fn stdin_path(&self) -> Option<&str>;
}

#[derive(Debug)]
pub enum ClockFrequency {
    U32(u32),
    U64(u64),
}

impl ClockFrequency {
    pub fn as_u64(&self) -> u64 {
        match self {
            ClockFrequency::U32(val) => *val as u64,
            ClockFrequency::U64(val) => *val,
        }
    }
}

#[derive(Debug)]
pub enum MiscellaneousProperty {
    ClockCells(u32),
    ClockFrequency(ClockFrequency),
    BootArgs(String),
    StdoutPath(String),
    StdinPath(String),
}

impl MiscellaneousProperty {
    pub fn as_str(&self) -> &str {
        match self {
            MiscellaneousProperty::ClockCells(_) => CLOCK_CELLS,
            MiscellaneousProperty::ClockFrequency(_) => CLOCK_FREQUENCY,
            MiscellaneousProperty::BootArgs(_) => BOOT_ARGS,
            MiscellaneousProperty::StdoutPath(_) => STDOUT_PATH,
            MiscellaneousProperty::StdinPath(_) => STDIN_PATH,
        }
    }
}
