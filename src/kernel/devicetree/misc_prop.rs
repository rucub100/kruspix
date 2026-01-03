// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

pub const CLOCK_FREQUENCY: &'static str = "clock-frequency";

#[derive(Debug)]
pub enum MiscPropError {}

pub type MiscPropResult<T> = Result<T, MiscPropError>;

pub trait MiscellaneousProperties {
    fn clock_frequency(&self) -> Option<&ClockFrequency>;
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
    ClockFrequency(ClockFrequency),
}

impl MiscellaneousProperty {
    pub fn as_str(&self) -> &str {
        match self {
            MiscellaneousProperty::ClockFrequency(_) => CLOCK_FREQUENCY,
        }
    }
}
