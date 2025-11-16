use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

/// A wrapper that allows global mutable access without atomics.
///
/// # Safety
/// This must ONLY be used during the single-core boot phase.
/// Once secondary cores are active, this becomes UNSAFE.
pub struct BootCell<T> {
    data: UnsafeCell<T>,
}

unsafe impl<T> Sync for BootCell<T> {}

impl<T> BootCell<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }

    /// Get exclusive access to the inner data.
    ///
    /// # Safety
    /// Caller must ensure no other reference exists (easy when single-core).
    pub fn lock(&self) -> &mut T {
        // in a real spinlock, we would atomic loop here
        // in this boot wrapper, we just hand it out
        unsafe { &mut *self.data.get() }
    }
}