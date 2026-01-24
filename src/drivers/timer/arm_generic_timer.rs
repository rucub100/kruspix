// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::string::String;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::misc_prop::{ClockFrequency, MiscellaneousProperties};
use crate::kernel::devicetree::node::Node;
use crate::kernel::irq::{enable_irq, resolve_virq};
use crate::kernel::time::{Alarm, Timer, register_global_timer, register_local_alarm};
use crate::kprintln;

pub struct GenericTimerDevice {
    id: String,
    clock_frequency: AtomicU64,
    virq: u32,
}

impl GenericTimerDevice {
    pub fn new(id: String, clock_frequency: u64, virq: u32) -> Self {
        GenericTimerDevice {
            id,
            clock_frequency: AtomicU64::new(clock_frequency),
            virq,
        }
    }
}

impl Device for GenericTimerDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, _node: &Node) -> Result<(), DriverInitError> {
        unsafe {
            let frequency: u64;
            // Counter-timer Frequency Register
            core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) frequency);
            if frequency > 0 {
                // override only if the register provides a valid frequency
                self.clock_frequency.store(frequency, Ordering::Release);
            }
        }

        register_global_timer(self);

        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        // Disable the virtual timer and mask its interrupt
        unsafe {
            // Virtual Timer Control Register
            // Bit 2: ISTATUS (Interrupt Status) - read only
            // Bit 1: IMASK (Interrupt Mask)
            // Bit 0: Enable
            core::arch::asm!("msr cntv_ctl_el0, {0}", in(reg) 0b10u64);
            // Virtual Timer TimerValue Register
            core::arch::asm!("msr cntv_tval_el0, {0}", in(reg) u32::MAX as u64);
        }

        enable_irq(self.virq).map_err(|_| DriverInitError::ToDo)?;
        register_local_alarm(self);

        Ok(())
    }
}

impl Timer for GenericTimerDevice {
    fn counter(&self) -> u64 {
        let counter: u64;
        unsafe {
            // CNTVCT_EL0: Virtual Count Register
            core::arch::asm!("isb");
            core::arch::asm!("mrs {0}, cntvct_el0", out(reg) counter);
        }
        counter
    }

    fn frequency_hz(&self) -> u64 {
        self.clock_frequency.load(Ordering::Acquire)
    }

    fn max_ticks(&self) -> u64 {
        u32::MAX as u64
    }
}

impl Alarm for GenericTimerDevice {
    fn schedule_at(&self, ticks: u64) {
        unsafe {
            core::arch::asm!("msr cntv_cval_el0, {0}", in(reg) ticks);
            core::arch::asm!("msr cntv_ctl_el0, {0}", in(reg) 0b1u64);
            core::arch::asm!("isb");
        }
    }

    fn virq(&self) -> u32 {
        self.virq
    }

    fn cancel(&self) {
        unsafe {
            core::arch::asm!("msr cntv_ctl_el0, {0}", in(reg) 0b10u64);
            core::arch::asm!("isb");
        }
    }

    fn frequency_hz(&self) -> u64 {
        self.clock_frequency.load(Ordering::Acquire)
    }

    fn max_ticks(&self) -> u64 {
        u32::MAX as u64
    }
}

pub struct GenericTimerDriver {
    dev_registry: DriverRegistry<GenericTimerDevice>,
}

impl PlatformDriver for GenericTimerDriver {
    fn compatible(&self) -> &[&str] {
        &["arm,armv7-timer"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        kprintln!("{:?} try init", self.compatible());

        let frequency = node.clock_frequency().unwrap_or(&ClockFrequency::U32(0));
        // according to devicetree bindings, index 2 corresponds to the virtual timer interrupt
        let virq = resolve_virq(node, 2).map_err(|_| DriverInitError::Retry)?;
        let dev = GenericTimerDevice::new(node.path(), frequency.as_u64(), virq);

        let dev = Arc::new(dev);

        dev.clone().global_setup(node)?;
        dev.clone().local_setup()?;

        self.dev_registry.add_device(node.path(), dev);

        kprintln!("{:?} initialized successfully", self.compatible());
        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

impl GenericTimerDriver {
    pub const fn new() -> Self {
        GenericTimerDriver {
            dev_registry: DriverRegistry::new(),
        }
    }
}

pub static DRIVER: GenericTimerDriver = GenericTimerDriver::new();
