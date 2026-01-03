// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::kernel::sync::SpinLock;

pub trait RestartHandler: Send + Sync {
    fn restart(&self);
}

pub trait PowerOffHandler: Send + Sync {
    fn power_off(&self);
}

static RESTART_HANDLERS: SpinLock<Vec<Arc<dyn RestartHandler>>> = SpinLock::new(Vec::new());
static POWER_OFF_HANDLERS: SpinLock<Vec<Arc<dyn PowerOffHandler>>> = SpinLock::new(Vec::new());

pub fn register_restart_handler(handler: Arc<dyn RestartHandler>) {
    let mut handlers = RESTART_HANDLERS.lock();
    handlers.push(handler);
}

pub fn register_power_off_handler(handler: Arc<dyn PowerOffHandler>) {
    let mut handlers = POWER_OFF_HANDLERS.lock();
    handlers.push(handler);
}

pub fn system_restart() {
    let handlers = RESTART_HANDLERS.lock();
    for handler in handlers.iter() {
        handler.restart();
    }
}

pub fn system_power_off() {
    let handlers = POWER_OFF_HANDLERS.lock();
    for handler in handlers.iter() {
        handler.power_off();
    }
}