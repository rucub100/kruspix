// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::time::Duration;

use crate::drivers::Device;
use crate::kernel::sync::SpinLock;

pub trait Watchdog: Device {
    fn is_running(&self) -> bool;
    fn start(&self);
    fn stop(&self);
    fn get_default_timeout(&self) -> Duration;
    fn get_max_timeout(&self) -> Duration;
    fn get_min_timeout(&self) -> Duration;
    fn set_timeout(&self, timeout: Duration);
    fn get_countdown(&self) -> Duration;
    fn acknowledge(&self);
}

static WATCHDOGS: SpinLock<Vec<Arc<dyn Watchdog>>> = SpinLock::new(Vec::new());

pub fn register_watchdog(wdt: Arc<dyn Watchdog>) {
    let mut watchdogs = WATCHDOGS.lock();
    watchdogs.push(wdt);
}
