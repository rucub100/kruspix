// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

mod once_lock;
mod spin_lock;

use crate::common::hash::FibonacciHash;
use crate::arch::cpu::{local_disable_interrupts, local_disable_irq_fiq, local_restore_interrupts};

pub(crate) use once_lock::OnceLock;
pub(crate) use spin_lock::SpinLock;
pub(crate) use spin_lock::SpinLockGuard;

const ADDR_LOCK_BITS: u32 = 8;
const ADDR_LOCK_COUNT: usize = 1 << ADDR_LOCK_BITS;
const ADDR_LOCK_COUNT_SHIFT: u32 = usize::BITS - ADDR_LOCK_BITS;

static GLOBAL_LOCK: SpinLock<()> = SpinLock::new(());
static ADDR_LOCKS: [SpinLock<()>; ADDR_LOCK_COUNT] = [const { SpinLock::new(()) }; ADDR_LOCK_COUNT];

#[inline]
pub fn with_global_lock<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = GLOBAL_LOCK.lock();
    f()
}

#[inline]
pub fn with_addr_lock<F, R>(addr: usize, f: F) -> R
where
    F: FnOnce() -> R,
{
    let index = addr.fibonacci_hash() >> ADDR_LOCK_COUNT_SHIFT;
    let _guard = ADDR_LOCKS[index].lock();
    f()
}

#[inline(always)]
pub fn without_interrupts<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let handle = local_disable_interrupts();
    let result = f();
    // SAFETY: We just disabled interrupts and saved the previous state in `handle`.
    unsafe { local_restore_interrupts(handle) };
    result
}

#[inline(always)]
pub fn without_irq_fiq<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let handle = local_disable_irq_fiq();
    let result = f();
    // SAFETY: We just disabled interrupts and saved the previous state in `handle`.
    unsafe { local_restore_interrupts(handle) };
    result
}