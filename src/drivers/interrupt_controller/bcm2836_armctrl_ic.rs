use alloc::string::String;
use alloc::sync::Arc;
use core::ptr::write_volatile;

use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::interrupts::{
    InterruptControllerNode, InterruptControllerOrNexusNode, InterruptGeneratingNode,
    InterruptSpecifier,
};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::irq::{
    InterruptController, InterruptHandler, IrqError, IrqResult, dispatch_irq, register_controller,
    register_handler, resolve_virq,
};
use crate::kernel::sync::OnceLock;
use crate::kprintln;
use crate::mm::map_io_region;

const HWIRQ_COUNT: u32 = 72;
const HWIRQ_BASIC_RANGE_START: u32 = 0;
const HWIRQ_BASIC_RANGE_END: u32 = 7;
const HWIRQ_1_RANGE_START: u32 = 8;
const HWIRQ_1_RANGE_END: u32 = 39;
const HWIRQ_2_RANGE_START: u32 = 40;
const HWIRQ_2_RANGE_END: u32 = 71;

// Register offsets
const IRQ_BASIC_PENDING_REG_OFFSET: usize = 0x0;
const IRQ_PENDING_1_REG_OFFSET: usize = 0x4;
const IRQ_PENDING_2_REG_OFFSET: usize = 0x8;
const FIQ_CTRL_REG_OFFSET: usize = 0xC;
const ENABLE_IRQS_1_REG_OFFSET: usize = 0x10;
const ENABLE_IRQS_2_REG_OFFSET: usize = 0x14;
const ENABLE_BASIC_IRQS_REG_OFFSET: usize = 0x18;
const DISABLE_IRQS_1_REG_OFFSET: usize = 0x1C;
const DISABLE_IRQS_2_REG_OFFSET: usize = 0x20;
const DISABLE_BASIC_IRQS_REG_OFFSET: usize = 0x24;

pub struct InterruptControllerDevice {
    path: String,
    reg_base: usize,
    virq_base: OnceLock<u32>,
}

impl Device for InterruptControllerDevice {
    fn global_setup(&self) {
        // disable FIQ
        let fiq_ctrl_reg = (self.reg_base + FIQ_CTRL_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(fiq_ctrl_reg, 0);
        }

        // disable all IRQs
        let disable_irqs_1_reg = (self.reg_base + DISABLE_IRQS_1_REG_OFFSET) as *mut u32;
        let disable_irqs_2_reg = (self.reg_base + DISABLE_IRQS_2_REG_OFFSET) as *mut u32;
        let disable_basic_irqs_reg = (self.reg_base + DISABLE_BASIC_IRQS_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(disable_irqs_1_reg, u32::MAX);
            write_volatile(disable_irqs_2_reg, u32::MAX);
            write_volatile(disable_basic_irqs_reg, u32::MAX);
        }
    }

    fn local_setup(&self) {}

    fn path(&self) -> &str {
        self.path.as_str()
    }
}

impl InterruptControllerDevice {
    fn new(node: &Node, reg_base: usize) -> Self {
        Self {
            path: node.path(),
            reg_base,
            virq_base: OnceLock::new(),
        }
    }

    fn enable_basic_irq(&self, hwirq: u32) {
        let enable_basic_irqs_reg = (self.reg_base + ENABLE_BASIC_IRQS_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(enable_basic_irqs_reg, 1 << hwirq);
        }
    }

    fn enable_irq_1(&self, hwirq: u32) {
        let enable_irqs_1_reg = (self.reg_base + ENABLE_IRQS_1_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(enable_irqs_1_reg, 1 << (hwirq - HWIRQ_1_RANGE_START));
        }
    }

    fn enable_irq_2(&self, hwirq: u32) {
        let enable_irqs_2_reg = (self.reg_base + ENABLE_IRQS_2_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(enable_irqs_2_reg, 1 << (hwirq - HWIRQ_2_RANGE_START));
        }
    }

    fn disable_basic_irq(&self, hwirq: u32) {
        let disable_basic_irqs_reg = (self.reg_base + DISABLE_BASIC_IRQS_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(disable_basic_irqs_reg, 1 << hwirq);
        }
    }

    fn disable_irq_1(&self, hwirq: u32) {
        let disable_irqs_1_reg = (self.reg_base + DISABLE_IRQS_1_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(disable_irqs_1_reg, 1 << (hwirq - HWIRQ_1_RANGE_START));
        }
    }

    fn disable_irq_2(&self, hwirq: u32) {
        let disable_irqs_2_reg = (self.reg_base + DISABLE_IRQS_2_REG_OFFSET) as *mut u32;
        unsafe {
            write_volatile(disable_irqs_2_reg, 1 << (hwirq - HWIRQ_2_RANGE_START));
        }
    }
}

impl InterruptController for InterruptControllerDevice {
    fn set_virq_base(&self, virq_base: u32) -> IrqResult<()> {
        self.virq_base
            .set(virq_base)
            .map_err(|_| IrqError::InvalidVirq)
    }

    fn enable(&self, hwirq: u32) {
        match hwirq {
            HWIRQ_BASIC_RANGE_START..=HWIRQ_BASIC_RANGE_END => self.enable_basic_irq(hwirq),
            HWIRQ_1_RANGE_START..=HWIRQ_1_RANGE_END => self.enable_irq_1(hwirq),
            HWIRQ_2_RANGE_START..=HWIRQ_2_RANGE_END => self.enable_irq_2(hwirq),
            _ => unreachable!(),
        }
    }

    fn disable(&self, hwirq: u32) {
        match hwirq {
            HWIRQ_BASIC_RANGE_START..=HWIRQ_BASIC_RANGE_END => self.disable_basic_irq(hwirq),
            HWIRQ_1_RANGE_START..=HWIRQ_1_RANGE_END => self.disable_irq_1(hwirq),
            HWIRQ_2_RANGE_START..=HWIRQ_2_RANGE_END => self.disable_irq_2(hwirq),
            _ => unreachable!(),
        }
    }

    fn pending_hwirq(&self) -> Option<u32> {
        let irq_basic_pending_reg = (self.reg_base + IRQ_BASIC_PENDING_REG_OFFSET) as *const u32;
        let pending_basic = unsafe { irq_basic_pending_reg.read_volatile() };

        if pending_basic == 0 {
            return None;
        }

        let next = pending_basic.trailing_zeros();
        match next {
            HWIRQ_BASIC_RANGE_START..=HWIRQ_BASIC_RANGE_END => Some(next),
            8 => {
                let irq_pending_1_reg = (self.reg_base + IRQ_PENDING_1_REG_OFFSET) as *const u32;
                let pending_1 = unsafe { irq_pending_1_reg.read_volatile() };

                // should not happen, but just in case
                if pending_1 == 0 {
                    return None;
                }

                Some(pending_1.trailing_zeros() + HWIRQ_1_RANGE_START)
            }
            9 => {
                let irq_pending_2_reg = (self.reg_base + IRQ_PENDING_2_REG_OFFSET) as *const u32;
                let pending_2 = unsafe { irq_pending_2_reg.read_volatile() };

                // should not happen, but just in case
                if pending_2 == 0 {
                    return None;
                }

                Some(pending_2.trailing_zeros() + HWIRQ_2_RANGE_START)
            }
            // selected interrupts
            10 => Some(next - 3 + HWIRQ_1_RANGE_START),
            11..=12 => Some(next - 2 + HWIRQ_1_RANGE_START),
            13..=14 => Some(next + 5 + HWIRQ_1_RANGE_START),
            15..=19 => Some(next + 38 + HWIRQ_1_RANGE_START),
            20 => Some(next + 42 + HWIRQ_2_RANGE_START),
            _ => unreachable!(),
        }
    }

    fn xlate(&self, specifier: &InterruptSpecifier) -> IrqResult<u32> {
        let bank = specifier.0.get(0).ok_or(IrqError::TranslationFailed)?;
        let int_num = specifier.0.get(1).ok_or(IrqError::TranslationFailed)?;

        match bank {
            0 => {
                if *int_num > HWIRQ_BASIC_RANGE_END {
                    return Err(IrqError::TranslationFailed);
                }
                Ok(*int_num)
            }
            1 => {
                let hwirq = *int_num + HWIRQ_1_RANGE_START;
                if hwirq > HWIRQ_1_RANGE_END {
                    return Err(IrqError::TranslationFailed);
                }
                Ok(hwirq)
            }
            2 => {
                let hwirq = *int_num + HWIRQ_2_RANGE_START;
                if hwirq > HWIRQ_2_RANGE_END {
                    return Err(IrqError::TranslationFailed);
                }
                Ok(hwirq)
            }
            _ => Err(IrqError::TranslationFailed),
        }
    }
}

impl InterruptHandler for InterruptControllerDevice {
    fn handle_irq(&self, _parent_virq: u32) {
        while let Some(hwirq) = self.pending_hwirq()
            && let Some(virq_base) = self.virq_base.get()
        {
            dispatch_irq(virq_base + hwirq);
            // SAFETY: acknowledge is a no-op for this controller
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
        "brcm,bcm2836-armctrl-ic"
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
        let dev = InterruptControllerDevice::new(node, addr);

        dev.global_setup();

        let dev = Arc::new(dev);

        register_controller(node, dev.clone(), HWIRQ_COUNT).map_err(|_| DriverInitError::Retry)?;

        if node.interrupts().is_some() || node.interrupts_extended().is_some() {
            let parent_virq = resolve_virq(node, 0).map_err(|_| DriverInitError::Retry)?;
            register_handler(parent_virq, dev).map_err(|_| DriverInitError::Retry)?;
        }

        kprintln!("[{}] initialized successfully", self.compatible());

        Ok(())
    }

    fn get_device(&self, node: &Node) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(node)
    }
}

pub static DRIVER: InterruptControllerDriver = InterruptControllerDriver::new();
