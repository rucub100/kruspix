use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// A simple spin lock implementation with no fairness guarantees.
///
/// # Safety
/// This spin lock does not implement any deadlock detection or prevention.
/// It doesn't disable interrupts while holding the lock, so it is the caller's responsibility.
pub struct SpinLock<T> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquires the spin lock and returns a guard that releases the lock when dropped.
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self.lock.swap(true, Ordering::Acquire) {
            spin_loop();
        }

        SpinLockGuard { lock: self }
    }

    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        if self.lock.swap(true, Ordering::Acquire) {
            None
        } else {
            Some(SpinLockGuard { lock: self })
        }
    }
}

// SAFETY: Sending a SpinLock to another thread is safe if T is Send.
// Sharing a SpinLock reference is safe because the lock enforces exclusive access.
unsafe impl<T: Send> Sync for SpinLock<T> {}
unsafe impl<T: Send> Send for SpinLock<T> {}

/// A guard that releases the spin lock when dropped.
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.lock.store(false, Ordering::Release);
    }
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}
