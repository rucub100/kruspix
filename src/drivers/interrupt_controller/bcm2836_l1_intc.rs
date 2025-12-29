use alloc::string::String;
use alloc::sync::Arc;
use core::mem::size_of;
use core::ptr::{read_volatile, write_volatile};

use crate::arch::cpu::core_id;
use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::interrupts::{
    InterruptControllerNode, InterruptControllerOrNexusNode, InterruptSpecifier,
};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::irq::{InterruptController, IrqError, IrqResult, register_controller};
use crate::kernel::sync::{with_addr_lock, without_irq_fiq};
use crate::kprintln;
use crate::mm::map_io_region;

mod hwirq {
    pub const COUNT: u32 = 12;
    // timer interrupts
    pub const HWIRQ_CNTPSIRQ: u32 = 0;
    pub const HWIRQ_CNTPNSIRQ: u32 = 1;
    pub const HWIRQ_CNTHPIRQ: u32 = 2;
    pub const HWIRQ_CNTVIRQ: u32 = 3;
    // mailbox interrupts
    pub const HWIRQ_MAILBOX_0: u32 = 4;
    pub const HWIRQ_MAILBOX_1: u32 = 5;
    pub const HWIRQ_MAILBOX_2: u32 = 6;
    pub const HWIRQ_MAILBOX_3: u32 = 7;
    // GPU interrupt
    pub const HWIRQ_GPU: u32 = 8;
    // PMU (performance monitoring unit) interrupt
    pub const HWIRQ_PMU: u32 = 9;
    // AXI interrupt
    pub const HWIRQ_AXI: u32 = 10;
    // Local timer interrupt
    pub const HWIRQ_LOCAL_TIMER: u32 = 11;
}

// Core un-related interrupts
const CORE_UNRELATED_DEST_IRQ_CORE_0: u32 = 0;

// Registers
const GPU_INT_ROUTING_REG_OFFSET: usize = 0xc;
const PMU_INT_ROUTING_SET_REG_OFFSET: usize = 0x10;
const PMU_INT_ROUTING_CLR_REG_OFFSET: usize = 0x14;
const LOCAL_TIMER_INT_ROUTING_REG_OFFSET: usize = 0x24;
const AXI_OUTSTANDING_IRQ_REG_OFFSET: usize = 0x30;
const LOCAL_TIMER_CTRL_AND_STATUS_REG_OFFSET: usize = 0x34;
const CORE_TIMERS_INT_CTRL_REG_OFFSET: usize = 0x40;
const CORE_MBOX_INT_CTRL_REG_OFFSET: usize = 0x50;
const CORE_IRQ_SOURCE_REG_OFFSET: usize = 0x60;

pub struct InterruptControllerDevice {
    id: String,
    reg_base: usize,
}

impl InterruptControllerDevice {
    fn new(id: String, reg_base: usize) -> Self {
        Self { id, reg_base }
    }

    #[inline]
    fn update_reg(&self, reg: *mut u32, bit: u32, enable: bool) {
        without_irq_fiq(|| {
            with_addr_lock(reg as usize, || unsafe {
                let mut value = read_volatile(reg);
                if enable {
                    value |= 1 << bit
                } else {
                    value &= !(1 << bit)
                };
                write_volatile(reg, value);
            });
        });
    }

    #[inline]
    fn update_core_related_reg(&self, reg_offset: usize, bit: u32, enable: bool) {
        let core_id = core_id();
        let core_related_reg =
            (self.reg_base + reg_offset + core_id * size_of::<u32>()) as *mut u32;
        self.update_reg(core_related_reg, bit, enable);
    }

    #[inline]
    fn update_core_unrelated_reg(&self, reg_offset: usize, bit: u32, enable: bool) {
        let core_unrelated_reg = (self.reg_base + reg_offset) as *mut u32;
        self.update_reg(core_unrelated_reg, bit, enable);
    }

    fn enable_timer_interrupt(&self, hwirq: u32) {
        // 4 core timer interrupts
        assert!(hwirq < 4);
        self.update_core_related_reg(CORE_TIMERS_INT_CTRL_REG_OFFSET, hwirq, true);
    }

    fn disable_timer_interrupt(&self, hwirq: u32) {
        // 4 core timer interrupts
        assert!(hwirq < 4);
        self.update_core_related_reg(CORE_TIMERS_INT_CTRL_REG_OFFSET, hwirq, false);
    }

    fn enable_mailbox_interrupt(&self, hwirq: u32) {
        // 4 core mailbox interrupts
        assert!(hwirq >= 4);
        assert!(hwirq < 8);
        self.update_core_related_reg(CORE_MBOX_INT_CTRL_REG_OFFSET, hwirq - 4, true);
    }

    fn disable_mailbox_interrupt(&self, hwirq: u32) {
        // 4 core mailbox interrupts
        assert!(hwirq >= 4);
        assert!(hwirq < 8);
        self.update_core_related_reg(CORE_MBOX_INT_CTRL_REG_OFFSET, hwirq - 4, false);
    }

    fn enable_pmu_interrupt(&self) {
        let core_id = core_id();
        let pmu_int_routing_set_reg = (self.reg_base + PMU_INT_ROUTING_SET_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(pmu_int_routing_set_reg, 1 << core_id);
        }
    }

    fn disable_pmu_interrupt(&self) {
        let core_id = core_id();
        let pmu_int_routing_clr_reg = (self.reg_base + PMU_INT_ROUTING_CLR_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(pmu_int_routing_clr_reg, 1 << core_id);
        }
    }

    fn enable_axi_interrupt(&self) {
        self.update_core_unrelated_reg(AXI_OUTSTANDING_IRQ_REG_OFFSET, 20, true);
    }

    fn disable_axi_interrupt(&self) {
        self.update_core_unrelated_reg(AXI_OUTSTANDING_IRQ_REG_OFFSET, 20, false);
    }

    fn enable_local_timer_interrupt(&self) {
        self.update_core_unrelated_reg(LOCAL_TIMER_CTRL_AND_STATUS_REG_OFFSET, 29, true);
    }

    fn disable_local_timer_interrupt(&self) {
        self.update_core_unrelated_reg(LOCAL_TIMER_CTRL_AND_STATUS_REG_OFFSET, 29, false);
    }
}

impl Device for InterruptControllerDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        // GPU interrupts routing to core 0 (FIQ + IRQ)
        let gpu_int_routing_reg = (self.reg_base + GPU_INT_ROUTING_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(gpu_int_routing_reg, 0);
        }

        // PMU interrupt routing (disabled)
        let pmu_int_routing_clr_reg = (self.reg_base + PMU_INT_ROUTING_CLR_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(pmu_int_routing_clr_reg, u32::MAX);
        }

        // AXI-outstanding interrupt enable (disabled)
        let axi_outstanding_irq_reg = (self.reg_base + AXI_OUTSTANDING_IRQ_REG_OFFSET) as *mut u32;
        with_addr_lock(axi_outstanding_irq_reg as usize, || unsafe {
            let mut value = read_volatile(axi_outstanding_irq_reg);
            value = value & !(1 << 20);
            write_volatile(axi_outstanding_irq_reg, value);
        });

        // Local timer interrupt routing to core 0 (IRQ)
        let local_timer_int_routing_reg =
            (self.reg_base + LOCAL_TIMER_INT_ROUTING_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(local_timer_int_routing_reg, CORE_UNRELATED_DEST_IRQ_CORE_0);
        }
        // Local timer interrupt enable (disabled)
        let local_timer_ctrl_and_status_reg =
            (self.reg_base + LOCAL_TIMER_CTRL_AND_STATUS_REG_OFFSET) as *mut u32;
        with_addr_lock(local_timer_ctrl_and_status_reg as usize, || unsafe {
            let mut value = read_volatile(local_timer_ctrl_and_status_reg);
            value = value & !(1 << 29);
            write_volatile(local_timer_ctrl_and_status_reg, value);
        });

        register_controller(node, self, hwirq::COUNT).map_err(|_| DriverInitError::Retry)?;

        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        let core_offset = core_id() * size_of::<u32>();

        // Core timer interrupt (disabled)
        let core_timers_int_ctrl_reg =
            (self.reg_base + CORE_TIMERS_INT_CTRL_REG_OFFSET + core_offset) as *mut u32;
        unsafe {
            write_volatile(core_timers_int_ctrl_reg, 0);
        }

        // Core mailbox interrupt (disabled)
        let core_mbox_int_ctrl_reg =
            (self.reg_base + CORE_MBOX_INT_CTRL_REG_OFFSET + core_offset) as *mut u32;
        unsafe {
            write_volatile(core_mbox_int_ctrl_reg, 0);
        }

        Ok(())
    }
}

impl InterruptController for InterruptControllerDevice {
    fn enable(&self, hwirq: u32) {
        match hwirq {
            hwirq::HWIRQ_GPU => {} // GPU interrupt is always enabled
            hwirq::HWIRQ_CNTPSIRQ
            | hwirq::HWIRQ_CNTPNSIRQ
            | hwirq::HWIRQ_CNTHPIRQ
            | hwirq::HWIRQ_CNTVIRQ => {
                self.enable_timer_interrupt(hwirq);
            }
            hwirq::HWIRQ_MAILBOX_0
            | hwirq::HWIRQ_MAILBOX_1
            | hwirq::HWIRQ_MAILBOX_2
            | hwirq::HWIRQ_MAILBOX_3 => {
                self.enable_mailbox_interrupt(hwirq);
            }
            hwirq::HWIRQ_PMU => {
                self.enable_pmu_interrupt();
            }
            hwirq::HWIRQ_AXI => {
                self.enable_axi_interrupt();
            }
            hwirq::HWIRQ_LOCAL_TIMER => {
                self.enable_local_timer_interrupt();
            }
            _ => unreachable!(),
        }
    }

    fn disable(&self, hwirq: u32) {
        match hwirq {
            hwirq::HWIRQ_GPU => {} // GPU interrupt is always enabled
            hwirq::HWIRQ_CNTPSIRQ
            | hwirq::HWIRQ_CNTPNSIRQ
            | hwirq::HWIRQ_CNTHPIRQ
            | hwirq::HWIRQ_CNTVIRQ => {
                self.disable_timer_interrupt(hwirq);
            }
            hwirq::HWIRQ_MAILBOX_0
            | hwirq::HWIRQ_MAILBOX_1
            | hwirq::HWIRQ_MAILBOX_2
            | hwirq::HWIRQ_MAILBOX_3 => {
                self.disable_mailbox_interrupt(hwirq);
            }
            hwirq::HWIRQ_PMU => {
                self.disable_pmu_interrupt();
            }
            hwirq::HWIRQ_AXI => {
                self.disable_axi_interrupt();
            }
            hwirq::HWIRQ_LOCAL_TIMER => {
                self.disable_local_timer_interrupt();
            }
            _ => unreachable!(),
        }
    }

    fn pending_hwirq(&self) -> Option<u32> {
        let core_id = core_id();

        let core_irq_source_reg =
            (self.reg_base + CORE_IRQ_SOURCE_REG_OFFSET + core_id * size_of::<u32>()) as *const u32;
        let pending = unsafe { read_volatile(core_irq_source_reg) };

        if pending == 0 {
            return None;
        }

        Some(pending.trailing_zeros())
    }

    /// Translates a specifier to a hardware IRQ number.
    ///
    /// # Safety
    /// This function assumes that the specifier length is 2, as verified during [`PlatformDriver::try_init`].
    fn xlate(&self, specifier: &InterruptSpecifier) -> IrqResult<u32> {
        // specifier[0]: hardware IRQ number
        // specifier[1]: flags (ignored)
        let hwirq = specifier.0.get(0).ok_or(IrqError::TranslationFailed)?;
        if *hwirq < hwirq::COUNT {
            Ok(*hwirq)
        } else {
            Err(IrqError::TranslationFailed)
        }
    }
}

pub struct InterruptControllerDriver {
    dev_registry: DriverRegistry<InterruptControllerDevice>,
}

impl InterruptControllerDriver {
    const fn new() -> Self {
        Self {
            dev_registry: DriverRegistry::new(),
        }
    }
}

impl PlatformDriver for InterruptControllerDriver {
    fn compatible(&self) -> &str {
        "brcm,bcm2836-l1-intc"
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        kprintln!("[{}] try init", self.compatible());

        if !node.is_interrupt_controller() {
            kprintln!(
                "[ERROR][{}] missing 'interrupt-controller' property",
                self.compatible()
            );
            return Err(DriverInitError::DeviceTreeError);
        }

        let interrupt_cells = node
            .interrupt_cells()
            .ok_or(DriverInitError::DeviceTreeError)?;
        if interrupt_cells != 2 {
            kprintln!(
                "[ERROR][{}] invalid '#interrupt-cells' property value: expected 2, got {}",
                self.compatible(),
                interrupt_cells
            );
            return Err(DriverInitError::DeviceTreeError);
        }

        let reg = node.reg().ok_or(DriverInitError::DeviceTreeError)?;
        if reg.len() != 1 {
            kprintln!(
                "[ERROR][{}] invalid 'reg' property: expected 1 region, got {}",
                self.compatible(),
                reg.len()
            );
            return Err(DriverInitError::DeviceTreeError);
        }

        let (phys_addr, length) = node
            .resolve_phys_address_and_length()
            .ok_or(DriverInitError::DeviceTreeError)?;
        kprintln!(
            "[{}] reg: phys_addr=0x{:x}, length=0x{:x}",
            self.compatible(),
            phys_addr,
            length
        );

        let addr = map_io_region(phys_addr, length);
        let dev = InterruptControllerDevice::new(node.path(), addr);
        let dev = Arc::new(dev);
        self.dev_registry.add_device(node.path(), dev.clone());

        dev.clone().global_setup(node)?;
        dev.local_setup()?;

        kprintln!("[{}] initialized successfully", self.compatible());

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

pub static DRIVER: InterruptControllerDriver = InterruptControllerDriver::new();
