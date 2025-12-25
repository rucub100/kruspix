use alloc::sync::Arc;
use alloc::vec::Vec;
use core::time::Duration;

use crate::kernel::irq::{InterruptHandler, register_handler};
use crate::kernel::sync::SpinLock;

const NANOS_PER_SEC: u128 = 1_000_000_000;

#[inline]
fn calc_resolution(frequency_hz: u128) -> Duration {
    let nanos = 1u64.max((NANOS_PER_SEC / frequency_hz) as u64);
    Duration::from_nanos(nanos)
}

#[inline]
fn convert_duration_to_ticks(frequency_hz: u128, duration: Duration) -> u64 {
    let secs = duration.as_secs() as u128;
    let nanos = duration.subsec_nanos() as u128;

    let secs_ticks = secs * frequency_hz;
    let nanos_ticks = (nanos * frequency_hz) / NANOS_PER_SEC;

    (secs_ticks + nanos_ticks) as u64
}

#[inline]
fn convert_ticks_to_duration(frequency_hz: u128, ticks: u64) -> Duration {
    let ticks = ticks as u128;
    let secs = ticks / frequency_hz;
    let nanos = ((ticks % frequency_hz) * NANOS_PER_SEC) / frequency_hz;
    Duration::new(secs as u64, nanos as u32)
}

pub trait Timer: Send + Sync {
    fn counter(&self) -> u64;
    fn frequency_hz(&self) -> u64;
    fn max_ticks(&self) -> u64;

    fn resolution(&self) -> Duration {
        calc_resolution(self.frequency_hz() as u128)
    }

    fn uptime(&self) -> Duration {
        self.ticks_to_duration(self.counter())
    }

    fn duration_to_ticks(&self, duration: Duration) -> u64 {
        convert_duration_to_ticks(self.frequency_hz() as u128, duration)
    }

    fn ticks_to_duration(&self, ticks: u64) -> Duration {
        convert_ticks_to_duration(self.frequency_hz() as u128, ticks)
    }
}

pub trait Alarm: Send + Sync {
    fn schedule_at(&self, ticks: u64);
    fn virq(&self) -> u32;
    fn cancel(&self);
    fn frequency_hz(&self) -> u64;
    fn max_ticks(&self) -> u64;

    fn resolution(&self) -> Duration {
        calc_resolution(self.frequency_hz() as u128)
    }

    fn min_duration(&self) -> Duration {
        self.resolution()
    }
    fn duration_to_ticks(&self, duration: Duration) -> u64 {
        convert_duration_to_ticks(self.frequency_hz() as u128, duration)
    }

    fn ticks_to_duration(&self, ticks: u64) -> Duration {
        convert_ticks_to_duration(self.frequency_hz() as u128, ticks)
    }
}

pub trait RealTimeClock: Send + Sync {}

static TIMERS: SpinLock<Vec<Arc<dyn Timer>>> = SpinLock::new(Vec::new());
static SYSTEM_TIMER: SpinLock<Option<Arc<dyn Timer>>> = SpinLock::new(None);
static ALARMS: SpinLock<Vec<Arc<dyn Alarm>>> = SpinLock::new(Vec::new());
static SYSTEM_ALARM: SpinLock<Option<Arc<dyn Alarm>>> = SpinLock::new(None);

pub fn register_timer(timer: Arc<dyn Timer>) {
    TIMERS.lock_irq().push(timer.clone());

    let mut system_timer = SYSTEM_TIMER.lock_irq();
    if match &*system_timer {
        Some(existing_timer) => timer.resolution() < existing_timer.resolution(),
        None => true,
    } {
        system_timer.replace(timer);
    }
}

pub fn register_alarm(alarm: Arc<dyn Alarm>) {
    ALARMS.lock_irq().push(alarm.clone());

    let mut system_alarm = SYSTEM_ALARM.lock_irq();
    if match &*system_alarm {
        Some(existing_alarm) => alarm.resolution() < existing_alarm.resolution(),
        None => true,
    } {
        system_alarm.replace(alarm);
    }
}

pub fn uptime() -> Duration {
    SYSTEM_TIMER
        .lock_irq()
        .as_ref()
        .map(|timer| timer.uptime())
        .unwrap_or(Duration::ZERO)
}

pub fn test_alarm(duration: Duration, callback: Arc<dyn InterruptHandler>) {
    let system_timer = SYSTEM_TIMER.lock_irq();
    let system_timer = system_timer.as_ref();
    let system_alarm = SYSTEM_ALARM.lock_irq();
    let system_alarm = system_alarm.as_ref();

    if let Some(system_timer) = system_timer
        && let Some(system_alarm) = system_alarm
    {
        system_alarm.cancel();
        register_handler(system_alarm.virq(), callback).unwrap();
        let ticks = system_alarm.duration_to_ticks(duration);
        let ticks_verified = system_alarm
            .duration_to_ticks(duration.max(system_alarm.min_duration()))
            .min(system_alarm.max_ticks());
        assert_eq!(ticks, ticks_verified);
        let ticks = system_timer.counter().wrapping_add(ticks) % system_alarm.max_ticks();
        system_alarm.schedule_at(ticks);
    }
}

pub fn cancel_alarm() {
    let system_alarm = SYSTEM_ALARM.lock_irq();
    if let Some(system_alarm) = system_alarm.as_ref() {
        system_alarm.cancel();
    }
}
