use core::cell::UnsafeCell;

/// A wrapper that allows global mutable access without atomics.
///
/// # Safety
/// This must ONLY be used during the single-core boot phase.
/// Once secondary cores are active, this becomes UNSAFE.
pub struct BootCell<T> {
    data: UnsafeCell<Option<T>>,
}

unsafe impl<T> Sync for BootCell<T> {}

impl<T> BootCell<T> {
    pub const fn new() -> Self {
        Self {
            data: UnsafeCell::new(None),
        }
    }

    pub fn init(&self, data: T) {
        let ptr = unsafe { &mut *self.data.get() };

        if ptr.is_some() {
            panic!("BootCell already initialized!");
        }

        *ptr = Some(data);
    }

    /// Get exclusive access to the inner data.
    ///
    /// # Safety
    /// Caller must ensure no other reference exists (easy when single-core).
    pub fn lock(&self) -> &mut T {
        // in a real spinlock, we would atomic loop here
        // in this boot wrapper, we just hand it out
        let ptr = unsafe { &mut *self.data.get() };

        match ptr {
            Some(data) => data,
            None => panic!("BootCell not initialized!"),
        }
    }
    
    pub fn try_lock(&self) -> Option<&mut T> {
        let ptr = unsafe { &mut *self.data.get() };

        match ptr {
            Some(data) => Some(data),
            None => None,
        }
    }
}
