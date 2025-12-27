use alloc::string::String;
use alloc::sync::Arc;

use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::{misc_prop::MiscellaneousProperties, std_prop::StandardProperties};
use crate::kernel::irq::resolve_virq;
use crate::kernel::time::{Alarm, Timer, register_global_alarm, register_global_timer};
use crate::kprintln;
use crate::mm::map_io_region;

/// Control/Status Register Offset
const CS_REG_OFFSET: u32 = 0x00;
/// Counter Register Offset (Lower 32 bits)
const CLO_REG_OFFSET: u32 = 0x04;
// Counter Register Offset (Higher 32 bits)
// const CHI_REG_OFFSET: u32 = 0x08;
/// Compare 3
///
/// # Safety
/// Channels 0 and 2 are used by the VideoCore GPU, so only channels 1 and 3 are safe
/// for use by the ARM CPU. Here we choose channel 3 just like the Linux kernel does.
const C3_REG_OFFSET: u32 = 0x18;
const CS_REG_M3_BIT: u32 = 1 << 3;

pub struct TimerDevice {
    id: String,
    reg_base: usize,
    clock_frequency: u64,
    virq: u32,
}

impl Device for TimerDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, _node: &Node) -> Result<(), DriverInitError> {
        // clear timer 3 channel comparator match
        // write 1 to clear the status and the corresponding irq line
        let cs_reg = (self.reg_base + CS_REG_OFFSET as usize) as *mut u32;
        unsafe {
            cs_reg.write_volatile(CS_REG_M3_BIT);
        }

        // reset the C3 compare register but don't touch others
        let c3_reg = (self.reg_base + C3_REG_OFFSET as usize) as *mut u32;
        unsafe {
            c3_reg.write_volatile(0);
        }

        register_global_timer(self.clone() as Arc<dyn Timer>);
        register_global_alarm(self.clone() as Arc<dyn Alarm>);

        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl TimerDevice {
    fn new(id: String, reg_base: usize, clock_frequency: u64, virq: u32) -> Self {
        TimerDevice {
            id,
            reg_base,
            clock_frequency,
            virq,
        }
    }

    fn get_lower_counter(&self) -> u32 {
        let clo_reg = (self.reg_base + CLO_REG_OFFSET as usize) as *const u32;
        unsafe { clo_reg.read_volatile() }
    }

    fn set_compare(&self, value: u32) {
        let c3_reg = (self.reg_base + C3_REG_OFFSET as usize) as *mut u32;
        unsafe {
            c3_reg.write_volatile(value);
        }
    }

    fn clear_irq(&self) {
        let cs_reg = (self.reg_base + CS_REG_OFFSET as usize) as *mut u32;
        unsafe {
            cs_reg.write_volatile(CS_REG_M3_BIT);
        }
    }
}

impl Timer for TimerDevice {
    fn counter(&self) -> u64 {
        self.get_lower_counter() as u64
    }

    fn frequency_hz(&self) -> u64 {
        self.clock_frequency
    }

    fn max_ticks(&self) -> u64 {
        u32::MAX as u64
    }
}

impl Alarm for TimerDevice {
    fn schedule_at(&self, ticks: u64) {
        assert!(ticks < Alarm::max_ticks(self));
        self.set_compare(ticks as u32);
    }

    fn virq(&self) -> u32 {
        self.virq
    }

    fn cancel(&self) {
        self.clear_irq();
    }

    fn frequency_hz(&self) -> u64 {
        self.clock_frequency
    }

    fn max_ticks(&self) -> u64 {
        u32::MAX as u64
    }
}

pub struct TimerDriver {
    dev_registry: DriverRegistry<TimerDevice>,
}

impl TimerDriver {
    const fn new() -> Self {
        Self {
            dev_registry: DriverRegistry::new(),
        }
    }
}

impl PlatformDriver for TimerDriver {
    fn compatible(&self) -> &str {
        "brcm,bcm2835-system-timer"
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        kprintln!("[{}] try init", self.compatible());

        let reg = node.reg().ok_or(DriverInitError::DeviceTreeError)?;
        if reg.len() != 1 {
            kprintln!(
                "[ERROR][{}] invalid 'reg' property: expected 1 region, got {}",
                self.compatible(),
                reg.len()
            );
            return Err(DriverInitError::DeviceTreeError);
        }

        let rate = node
            .clock_frequency()
            .map_or(1_000_000u64, |cf| cf.as_u64());

        let (phys_addr, length) = node
            .resolve_phys_address_and_length()
            .ok_or(DriverInitError::DeviceTreeError)?;
        kprintln!(
            "[{}] reg: phys_addr=0x{:x}, length=0x{:x}",
            self.compatible(),
            phys_addr,
            length
        );

        let virq = resolve_virq(node, 3).map_err(|_| DriverInitError::Retry)?;

        let addr = map_io_region(phys_addr, length);
        let dev = TimerDevice::new(node.path(), addr, rate, virq);
        let dev = Arc::new(dev);

        dev.global_setup(node)?;

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

pub static DRIVER: TimerDriver = TimerDriver::new();
