use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicU8, Ordering};

const UNINITIALIZED: u8 = 0;
const INITIALIZING: u8 = 1;
const INITIALIZED: u8 = 2;

pub struct OnceLock<T> {
    status: AtomicU8,
    data: UnsafeCell<Option<T>>,
}

impl<T> OnceLock<T> {
    pub const fn new() -> Self {
        Self {
            status: AtomicU8::new(UNINITIALIZED),
            data: UnsafeCell::new(None),
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.status.load(Ordering::Acquire) == INITIALIZED {
            return unsafe { (*self.data.get()).as_ref() };
        }

        None
    }

    pub fn set(&self, value: T) -> Result<(), T> {
        let mut status = self.status.load(Ordering::Acquire);
        loop {
            match status {
                INITIALIZED => {
                    return Err(value);
                }
                INITIALIZING => {
                    spin_loop();
                    status = self.status.load(Ordering::Acquire);
                }
                UNINITIALIZED => {
                    match self.status.compare_exchange(
                        UNINITIALIZED,
                        INITIALIZING,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            unsafe {
                                *self.data.get() = Some(value);
                            }
                            self.status.store(INITIALIZED, Ordering::Release);
                            return Ok(());
                        }
                        Err(current_status) => {
                            status = current_status;
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}


unsafe impl<T: Sync + Send> Sync for OnceLock<T> {}
unsafe impl<T: Send> Send for OnceLock<T> {}
