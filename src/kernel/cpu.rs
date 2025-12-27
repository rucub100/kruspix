use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::arch::cpu::{core_id, get_local, set_local};
use crate::kernel::sync::OnceLock;
use crate::kernel::time::Alarm;

pub struct LocalData {
    core_id: usize,
    alarm: OnceLock<Arc<dyn Alarm>>,
}

impl LocalData {
    const fn new(core_id: usize) -> Self {
        LocalData {
            core_id,
            alarm: OnceLock::new(),
        }
    }

    pub fn core_id(&self) -> usize {
        self.core_id
    }

    pub fn set_alarm(&self, alarm: Arc<dyn Alarm>) -> Result<(), Arc<dyn Alarm>> {
        self.alarm.set(alarm)
    }

    pub fn get_alarm(&self) -> Option<Arc<dyn Alarm>> {
        self.alarm.get().cloned()
    }
}

pub fn init_local_data() {
    let cpu_id = core_id();
    let local_data = Box::leak(Box::new(LocalData::new(cpu_id)));

    unsafe {
        set_local(local_data);
    }
}

pub fn get_local_data() -> &'static LocalData {
    unsafe { get_local() }
}
