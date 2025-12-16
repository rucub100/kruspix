mod once_lock;
mod spin_lock;

use crate::common::hash::FibonacciHash;
use crate::arch::cpu::{disable_interrupts, disable_irq_fiq, restore_interrupts};

pub(crate) use once_lock::OnceLock;
pub(crate) use spin_lock::SpinLock;

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
    let handle = disable_interrupts();
    let result = f();
    // SAFETY: We just disabled interrupts and saved the previous state in `handle`.
    unsafe { restore_interrupts(handle) };
    result
}

#[inline(always)]
pub fn without_irq_fiq<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let handle = disable_irq_fiq();
    let result = f();
    // SAFETY: We just disabled interrupts and saved the previous state in `handle`.
    unsafe { restore_interrupts(handle) };
    result
}