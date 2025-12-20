use alloc::sync::Arc;

use crate::drivers::{DriverInitError, PlatformDriver};
use crate::kernel::devicetree::interrupts::{
    InterruptControllerNode, InterruptControllerOrNexusNode, InterruptGeneratingNode,
    InterruptSpecifier,
};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::irq::{
    InterruptController, InterruptHandler, IrqResult, dispatch_irq, register_controller,
    register_handler, resolve_virq,
};
use crate::kernel::sync::OnceLock;
use crate::kprintln;
use crate::mm::map_io_region;

// IRQ basic pending
mod bank0 {
    pub const ARM_TIMER: u32 = 0;
    pub const ARM_MAILBOX: u32 = 1;
    pub const ARM_DOORBELL_0: u32 = 2;
    pub const ARM_DOORBELL_1: u32 = 3;
    pub const VPU0_HALTED: u32 = 4;
    pub const VPU1_HALTED: u32 = 5;
    pub const ILLEGAL_TYPE0: u32 = 6;
    pub const ILLEGAL_TYPE1: u32 = 7;
}

// IRQ pending 1
mod bank1 {
    pub const TIMER0: u32 = 0;
    pub const TIMER1: u32 = 1;
    pub const TIMER2: u32 = 2;
    pub const TIMER3: u32 = 3;
    pub const CODEC0: u32 = 4;
    pub const CODEC1: u32 = 5;
    pub const CODEC2: u32 = 6;
    pub const VC_JPEG: u32 = 7;
    pub const ISP: u32 = 8;
    pub const VC_USB: u32 = 9;
    pub const VC_3D: u32 = 10;
    pub const TRANSPOSER: u32 = 11;
    pub const MULTICORESYNC0: u32 = 12;
    pub const MULTICORESYNC1: u32 = 13;
    pub const MULTICORESYNC2: u32 = 14;
    pub const MULTICORESYNC3: u32 = 15;
    pub const DMA0: u32 = 16;
    pub const DMA1: u32 = 17;
    pub const VC_DMA2: u32 = 18;
    pub const VC_DMA3: u32 = 19;
    pub const DMA4: u32 = 20;
    pub const DMA5: u32 = 21;
    pub const DMA6: u32 = 22;
    pub const DMA7: u32 = 23;
    pub const DMA8: u32 = 24;
    pub const DMA9: u32 = 25;
    pub const DMA10: u32 = 26;
    // shared interrupt for DMA 11 to 14
    pub const DMA11_14: u32 = 27;
    // triggers on all dma interrupts (including chanel 15)
    pub const DMAALL: u32 = 28;
    pub const AUX: u32 = 29;
    pub const ARM: u32 = 30;
    pub const VPUDMA: u32 = 31;
}

// IRQ pending 2
mod bank2 {
    pub const HOSTPORT: u32 = 0;
    pub const VIDEOSCALER: u32 = 1;
    pub const CCP2TX: u32 = 2;
    pub const SDC: u32 = 3;
    pub const DSI0: u32 = 4;
    pub const AVE: u32 = 5;
    pub const CAM0: u32 = 6;
    pub const CAM1: u32 = 7;
    pub const HDMI0: u32 = 8;
    pub const HDMI1: u32 = 9;
    pub const PIXELVALVE1: u32 = 10;
    pub const I2CSPISLV: u32 = 11;
    pub const DSI1: u32 = 12;
    pub const PWA0: u32 = 13;
    pub const PWA1: u32 = 14;
    pub const CPR: u32 = 15;
    pub const SMI: u32 = 16;
    pub const GPIO0: u32 = 17;
    pub const GPIO1: u32 = 18;
    pub const GPIO2: u32 = 19;
    pub const GPIO3: u32 = 20;
    pub const VC_I2C: u32 = 21;
    pub const VC_SPI: u32 = 22;
    pub const VC_I2SPCM: u32 = 23;
    pub const VC_SDIO: u32 = 24;
    pub const VC_UART: u32 = 25;
    pub const SLIMBUS: u32 = 26;
    pub const VEC: u32 = 27;
    pub const CPG: u32 = 28;
    pub const RNG: u32 = 29;
    pub const VC_ARASANSDIO: u32 = 30;
    pub const AVSPMON: u32 = 31;
}

const HWIRQ_COUNT: u32 = 72;

// Register offsets
const IRQ_BASIC_PENDING_REG_OFFSET: usize = 0x200;
const IRQ_PENDING_1_REG_OFFSET: usize = 0x204;
const IRQ_PENDING_2_REG_OFFSET: usize = 0x208;
const FIQ_CTRL_REG_OFFSET: usize = 0x20C;
const ENABLE_IRQS_1_REG_OFFSET: usize = 0x210;
const ENABLE_IRQS_2_REG_OFFSET: usize = 0x214;
const ENABLE_BASIC_IRQS_REG_OFFSET: usize = 0x218;
const DISABLE_IRQS_1_REG_OFFSET: usize = 0x21C;
const DISABLE_IRQS_2_REG_OFFSET: usize = 0x220;
const DISABLE_BASIC_IRQS_REG_OFFSET: usize = 0x224;

pub struct InterruptControllerDriver;

pub struct InterruptControllerDevice {
    reg_base: usize,
    virq_base: OnceLock<u32>,
}

impl InterruptControllerDevice {
    fn init(reg_base: usize) -> Self {
        // disable FIQ
        let fiq_ctrl_reg = (reg_base + FIQ_CTRL_REG_OFFSET) as *mut u32;
        unsafe {
            fiq_ctrl_reg.write_volatile(0);
        }

        // disable all IRQs
        let disable_irqs_1_reg = (reg_base + DISABLE_IRQS_1_REG_OFFSET) as *mut u32;
        let disable_irqs_2_reg = (reg_base + DISABLE_IRQS_2_REG_OFFSET) as *mut u32;
        let disable_basic_irqs_reg = (reg_base + DISABLE_BASIC_IRQS_REG_OFFSET) as *mut u32;
        unsafe {
            disable_irqs_1_reg.write_volatile(u32::MAX);
            disable_irqs_2_reg.write_volatile(u32::MAX);
            disable_basic_irqs_reg.write_volatile(u32::MAX);
        }

        Self {
            reg_base,
            virq_base: OnceLock::new(),
        }
    }
}

impl InterruptController for InterruptControllerDevice {
    fn set_virq_base(&self, virq_base: u32) {
        let _ = self.virq_base.set(virq_base);
    }

    fn enable(&self, hwirq: u32) {
        todo!()
    }

    fn disable(&self, hwirq: u32) {
        todo!()
    }

    fn pending_hwirq(&self) -> Option<u32> {
        todo!()
    }

    fn ack(&self, hwirq: u32) {
        todo!()
    }

    fn xlate(&self, specifier: &InterruptSpecifier) -> IrqResult<u32> {
        todo!()
    }
}

impl InterruptHandler for InterruptControllerDevice {
    fn handle_irq(&self, _parent_virq: u32) {
        while let Some(hwirq) = self.pending_hwirq()
            && let Some(virq_base) = self.virq_base.get()
        {
            dispatch_irq(virq_base + hwirq);
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

        let reg = reg.first().ok_or(DriverInitError::DeviceTreeError)?;
        let phys_addr = reg
            .address_as_usize()
            .map_err(|_| DriverInitError::DeviceTreeError)?;
        let length = reg
            .length_as_usize()
            .map_err(|_| DriverInitError::DeviceTreeError)?;

        kprintln!(
            "[{}] reg: phys_addr=0x{:x}, length=0x{:x}",
            self.compatible(),
            phys_addr,
            length
        );

        let addr = map_io_region(phys_addr, length);
        let dev = InterruptControllerDevice::init(addr);
        let dev = Arc::new(dev);

        register_controller(node, dev.clone(), HWIRQ_COUNT).map_err(|_| DriverInitError::Retry)?;

        if node.interrupts().is_some() {
            let parent_virq = resolve_virq(node, 0).map_err(|_| DriverInitError::Retry)?;
            register_handler(parent_virq, dev).map_err(|_| DriverInitError::Retry)?;
        }

        kprintln!("[{}] initialized successfully", self.compatible());

        Ok(())
    }
}

pub static DRIVER: InterruptControllerDriver = InterruptControllerDriver;
