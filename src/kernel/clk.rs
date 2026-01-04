// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::drivers::Device;
use crate::kernel::sync::SpinLock;

#[derive(Debug, PartialEq, Eq)]
pub enum ClockError {
    RateNotSupported,
    ParentNotReady,
    HardwareTimeout,
    PermissionDenied,
}

pub type ClockResult<T> = Result<T, ClockError>;

pub trait Clock: Device + Send + Sync {
    fn name(&self) -> &str;
    fn prepare(&self) -> ClockResult<()>;
    fn enable(&self) -> ClockResult<()>;
    fn disable(&self);
    fn unprepare(&self);
    fn get_rate(&self) -> u64;
    fn set_rate(&self, hz: u64) -> ClockResult<()>;
}

static CLOCKS: SpinLock<Vec<Arc<dyn Clock>>> = SpinLock::new(Vec::new());

pub fn register_clock(clock: Arc<dyn Clock>) {
    CLOCKS.lock().push(clock);
}
