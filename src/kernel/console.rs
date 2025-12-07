use alloc::boxed::Box;
use alloc::vec::Vec;

use super::sync::spinlock::SpinLock;

pub trait Console: Send + Sync {
    fn write(&self, s: &str);
}

static SYSTEM_CONSOLE: SpinLock<Vec<Box<dyn Console>>> = SpinLock::new(Vec::new());
// static EARLY_CONSOLE: SpinLock<???> = ???