// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;

use crate::drivers::Device;
use crate::kernel::devicetree::node::Node;
use crate::kernel::sync::SpinLock;

#[derive(Debug, PartialEq, Eq)]
pub enum ClockError {
    RateNotSupported,
    ParentNotReady,
    HardwareTimeout,
    PermissionDenied,
}

pub type ClockResult<T> = Result<T, ClockError>;

pub trait Clock: Device {
    fn name(&self) -> &str;
    fn startup(&self) -> ClockResult<()>;
    fn shutdown(&self) -> ClockResult<()>;
    fn enable(&self) -> ClockResult<()>;
    fn disable(&self) -> ClockResult<()>;
    fn get_rate(&self) -> u64;
    fn set_rate(&self, hz: u64) -> ClockResult<()>;
}

struct ClockDomain {
    dev: Arc<dyn Clock>,
    parent: Option<Arc<ClockDomain>>,
    prepare_count: AtomicUsize,
    enable_count: AtomicUsize,
}

impl ClockDomain {
    const fn new(dev: Arc<dyn Clock>, parent: Option<Arc<ClockDomain>>) -> Self {
        Self {
            dev,
            parent,
            prepare_count: AtomicUsize::new(0),
            enable_count: AtomicUsize::new(0),
        }
    }
}

static CLOCKS: SpinLock<Vec<Arc<ClockDomain>>> = SpinLock::new(Vec::new());

pub fn register_clock(node: &Node, clock: Arc<dyn Clock>) -> ClockResult<()> {
    // TODO: how about parent clocks?

    CLOCKS
        .lock_irq()
        .push(Arc::new(ClockDomain::new(clock, None)));

    Ok(())
}
