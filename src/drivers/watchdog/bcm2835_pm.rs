// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! BCM2835 Power Management and Watchdog Timer Driver
//!
//! This driver was developed using the following Linux kernel sources as a technical
//! reference for hardware register layouts and sequencing:
//!
//! * <https://github.com/raspberrypi/linux/blob/rpi-6.12.y/drivers/mfd/bcm2835-pm.c>
//! * <https://github.com/raspberrypi/linux/blob/rpi-6.12.y/drivers/watchdog/bcm2835_wdt.c>
//! * <https://github.com/raspberrypi/linux/blob/rpi-6.12.y/drivers/pmdomain/bcm/bcm2835-power.c>
//!
//! While the Linux kernel is licensed under GPL-2.0, this independent implementation
//! is provided under the MIT License. It does not contain code copied from the
//! reference sources but utilizes the hardware specifications derived from them.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;

use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::prop::PropertyValue;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::sync::SpinLock;
use crate::kernel::time::{convert_duration_to_ticks, convert_ticks_to_duration};
use crate::kernel::watchdog::{Watchdog, register_watchdog};
use crate::kprintln;
use crate::mm::map_io_region;

mod wdt {
    pub const PM_RSTC_REG_OFFSET: usize = 0x1c;
    pub const PM_RSTS_REG_OFFSET: usize = 0x20;
    pub const PM_WDOG_REG_OFFSET: usize = 0x24;

    pub const PM_WDOG_FREQUENCY_HZ: u128 = 1 << 16;
    /// Watchdog time value mask (20 bits)
    pub const PM_WDOG_TIME_SET: u32 = 0x000fffff;
    pub const PM_RSTC_WRCFG_CLR: u32 = 0xffffffcf;
    pub const PM_RSTS_HADWRH_SET: u32 = 0x00000040;
    pub const PM_RSTC_WRCFG_SET: u32 = 0x00000030;
    pub const PM_RSTC_WRCFG_FULL_RESET: u32 = 0x00000020;
    pub const PM_RSTC_RESET: u32 = 0x00000102;
    pub const PM_RSTS_PARTITION_CLR: u32 = 0xfffffaaa;
}

mod power {
    // TODO
}

const PM_PASSWORD: u32 = 0x5a000000;

pub struct WatchdogTimerDevice {
    id: String,
    reg_base: usize,
    timeout: AtomicU32,
    lock: SpinLock<()>,
}

impl WatchdogTimerDevice {
    fn new(id: String, reg_base: usize) -> Self {
        WatchdogTimerDevice {
            id,
            reg_base,
            timeout: AtomicU32::new(15),
            lock: SpinLock::new(()),
        }
    }

    fn timeout_ticks(&self) -> u32 {
        let timeout_secs = self.timeout.load(Ordering::Acquire);
        convert_duration_to_ticks(
            wdt::PM_WDOG_FREQUENCY_HZ,
            Duration::from_secs(timeout_secs as u64),
        ) as u32
    }

    fn ticks_to_duration(&self, ticks: u32) -> Duration {
        let ticks = ticks & wdt::PM_WDOG_TIME_SET;
        convert_ticks_to_duration(wdt::PM_WDOG_FREQUENCY_HZ, ticks as u64)
    }
}

impl Device for WatchdogTimerDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        if let Some(prop) = node.properties().iter().find(|p| p.name() == "timeout-sec")
            && let Some(timeout_sec) = match prop.value() {
                PropertyValue::Unknown(prop) => prop.try_into().ok(),
                _ => None,
            }
        {
            let timeout_sec: u32 = timeout_sec;
            self.timeout.store(timeout_sec, Ordering::Release);
        }

        // DO NOT start the WDT if not running already (start doesn't work in QEMU)
        if self.is_running() {
            // FIXME: take over the watchdog from the firmware if it's running
            kprintln!("[{}] watchdog is running, stopping it", self.id());
            self.stop();
        }

        register_watchdog(self.clone());

        // TODO: register the system power-off and restart handlers

        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl Watchdog for WatchdogTimerDevice {
    fn is_running(&self) -> bool {
        let pm_rstc_reg = (self.reg_base + wdt::PM_RSTC_REG_OFFSET) as *const u32;
        unsafe {
            let rstc = pm_rstc_reg.read_volatile();
            (rstc & wdt::PM_RSTC_WRCFG_FULL_RESET) != 0
        }
    }

    fn start(&self) {
        let _guard = self.lock.lock_irq();
        let pm_wdog_reg = (self.reg_base + wdt::PM_WDOG_REG_OFFSET) as *mut u32;
        let pm_rstc_reg = (self.reg_base + wdt::PM_RSTC_REG_OFFSET) as *mut u32;

        unsafe {
            // write timeout value
            let timeout_ticks = self.timeout_ticks();
            pm_wdog_reg.write_volatile((timeout_ticks & wdt::PM_WDOG_TIME_SET) | PM_PASSWORD);
            // configure full reset
            let pm_rstc = pm_rstc_reg.read_volatile();
            pm_rstc_reg.write_volatile(
                (pm_rstc & wdt::PM_RSTC_WRCFG_CLR) | wdt::PM_RSTC_WRCFG_FULL_RESET | PM_PASSWORD,
            );
        }
    }

    fn stop(&self) {
        let pm_rstc_reg = (self.reg_base + wdt::PM_RSTC_REG_OFFSET) as *mut u32;
        unsafe {
            pm_rstc_reg.write_volatile(wdt::PM_RSTC_RESET | PM_PASSWORD);
        }
    }

    fn get_default_timeout(&self) -> Duration {
        let timeout_secs = self.timeout.load(Ordering::Acquire);
        Duration::from_secs(timeout_secs as u64)
    }

    fn get_max_timeout(&self) -> Duration {
        Duration::from_secs(15)
    }

    fn get_min_timeout(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn set_timeout(&self, timeout: Duration) {
        let secs = timeout.as_secs();
        assert!((1..=15).contains(&secs));

        self.timeout.store(secs as u32, Ordering::Release);
    }

    fn get_countdown(&self) -> Duration {
        let pm_wdog_reg = (self.reg_base + wdt::PM_WDOG_REG_OFFSET) as *mut u32;

        unsafe {
            let timeout_ticks = pm_wdog_reg.read_volatile();
            self.ticks_to_duration(timeout_ticks)
        }
    }

    fn acknowledge(&self) {
        let pm_wdog_reg = (self.reg_base + wdt::PM_WDOG_REG_OFFSET) as *mut u32;

        unsafe {
            let timeout_ticks = self.timeout_ticks();
            pm_wdog_reg.write_volatile((timeout_ticks & wdt::PM_WDOG_TIME_SET) | PM_PASSWORD);
        }
    }
}

pub struct WatchdogTimerAndPowerDriver {
    dev_registry: DriverRegistry<WatchdogTimerDevice>,
}

impl PlatformDriver for WatchdogTimerAndPowerDriver {
    fn compatible(&self) -> &[&str] {
        // FIXME: replace with brcm,bcm2835-pm once power management is implemented
        &["brcm,bcm2835-pm-wdt"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        kprintln!("{:?} try init", self.compatible());

        let reg = node.reg().ok_or(DriverInitError::DeviceTreeError)?;
        if !(2..=3).contains(&reg.len()) {
            kprintln!(
                "[ERROR]{:?} invalid 'reg' property: expected 2-3 regions, got {}",
                self.compatible(),
                reg.len()
            );
            return Err(DriverInitError::DeviceTreeError);
        }

        let mut reg_index = 0;

        if let Some(reg_names) = node.properties().iter().find(|p| p.name() == "reg-names")
            && let Some(reg_names) = match reg_names.value() {
                PropertyValue::Unknown(prop) => prop.try_into().ok(),
                _ => None,
            }
        {
            let list: Vec<String> = reg_names;
            if let Some(index) = list.iter().position(|name| name == "pm") {
                reg_index = index;
            }
        }

        let (phys_addr, length) = node
            .resolve_phys_address_and_length(reg_index)
            .ok_or(DriverInitError::DeviceTreeError)?;
        kprintln!(
            "{:?} reg: phys_addr=0x{:x}, length=0x{:x}",
            self.compatible(),
            phys_addr,
            length
        );

        let addr = map_io_region(phys_addr, length);
        let dev = WatchdogTimerDevice::new(node.path(), addr);
        let dev = Arc::new(dev);

        self.dev_registry.add_device(node.path(), dev.clone());

        dev.clone().global_setup(node)?;

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

impl WatchdogTimerAndPowerDriver {
    const fn new() -> Self {
        WatchdogTimerAndPowerDriver {
            dev_registry: DriverRegistry::new(),
        }
    }
}

pub static DRIVER: WatchdogTimerAndPowerDriver = WatchdogTimerAndPowerDriver::new();
