// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! Documentation for the Broadcom BCM2835 VideoCore mailbox IPC can be found on the
//! [Mailboxes Wiki](https://github.com/raspberrypi/firmware/wiki/Mailboxes).

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::drivers::mailbox::{Mailbox, MailboxError, MailboxResult, register_mailbox};
use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::interrupts::InterruptGeneratingNode;
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
use crate::kernel::irq::{InterruptHandler, enable_irq, register_handler, resolve_virq};
use crate::kernel::sync::SpinLock;
use crate::mm::map_io_region;

/// This mailbox is for communication from VideoCore to ARM.
/// ARM must not write to this mailbox but can read from it to receive messages from VideoCore.
/// Interrupts are triggered for this mailbox.
const MBOX_0_REG_OFFSET: usize = 0x0;

/// This mailbox is for communication from ARM to VideoCore.
/// ARM must not read from this mailbox but can write to it to send messages to VideoCore.
const MBOX_1_REG_OFFSET: usize = 0x20;

// Mailbox register offsets

const MBOX_RW_OFFSET: usize = 0x0;
const MBOX_PEEK_OFFSET: usize = 0x10;
const MBOX_SENDER_OFFSET: usize = 0x14;
const MBOX_STATUS_OFFSET: usize = 0x18;
const MBOX_CONFIG_OFFSET: usize = 0x1c;

// Mailbox 0 channels

const MBOX_0_CH_0_PM: u32 = 0;
const MBOX_0_CH_1_FRAMEBUFFER: u32 = 1;
const MBOX_0_CH_2_VIRT_UART: u32 = 2;
const MBOX_0_CH_3_VCHIQ: u32 = 3;
const MBOX_0_CH_4_LED: u32 = 4;
const MBOX_0_CH_5_BUTTON: u32 = 5;
const MBOX_0_CH_6_TOUCH_SCREEN: u32 = 6;
const MBOX_0_CH_7: u32 = 7;
const MBOX_0_CH_8_ARM_VC_TAGS: u32 = 8;
const MBOX_0_CH_9_VC_ARM_TAGS: u32 = 9;

// FIFO status register flags

const MBOX_STATUS_EMPTY: u32 = 0x4000_0000;
const MBOX_STATUS_FULL: u32 = 0x8000_0000;

struct MailboxDevice {
    id: String,
    reg_base: usize,
    enabled: AtomicBool,
    lock: SpinLock<()>,
    buffer: Arc<SpinLock<VecDeque<u32>>>,
}

impl MailboxDevice {
    fn new(id: String, reg_base: usize) -> Self {
        Self {
            id,
            reg_base,
            enabled: AtomicBool::new(false),
            lock: SpinLock::new(()),
            buffer: Arc::new(SpinLock::new(VecDeque::new())),
        }
    }

    #[inline(always)]
    fn enable_interrupts(&self) {
        let mbox_0_config_reg =
            (self.reg_base + MBOX_0_REG_OFFSET + MBOX_CONFIG_OFFSET) as *mut u32;
        unsafe {
            mbox_0_config_reg.write_volatile(0x1);
        }
    }

    #[inline(always)]
    fn disable_interrupts(&self) {
        let mbox_0_config_reg =
            (self.reg_base + MBOX_0_REG_OFFSET + MBOX_CONFIG_OFFSET) as *mut u32;
        unsafe {
            mbox_0_config_reg.write_volatile(0x0);
        }
    }

    #[inline(always)]
    fn read_mbox_0(&self) -> u32 {
        let mbox_0_rw_reg = (self.reg_base + MBOX_0_REG_OFFSET + MBOX_RW_OFFSET) as *const u32;
        unsafe { mbox_0_rw_reg.read_volatile() }
    }

    #[inline(always)]
    fn write_mbox_1(&self, data: u32) {
        let mbox_1_rw_reg = (self.reg_base + MBOX_1_REG_OFFSET + MBOX_RW_OFFSET) as *mut u32;
        unsafe { mbox_1_rw_reg.write_volatile(data) }
    }

    #[inline(always)]
    fn get_mbox_0_status(&self) -> u32 {
        let status_reg = (self.reg_base + MBOX_0_REG_OFFSET + MBOX_STATUS_OFFSET) as *const u32;
        unsafe { status_reg.read_volatile() }
    }

    #[inline(always)]
    fn get_mbox_1_status(&self) -> u32 {
        let status_reg = (self.reg_base + MBOX_1_REG_OFFSET + MBOX_STATUS_OFFSET) as *const u32;
        unsafe { status_reg.read_volatile() }
    }
}

impl Device for MailboxDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        if node.interrupts().is_some() || node.interrupts_extended().is_some() {
            let virq = resolve_virq(node, 0).map_err(|_| DriverInitError::Retry)?;
            register_handler(virq, self.clone()).map_err(|_| DriverInitError::Retry)?;
            enable_irq(virq).map_err(|_| DriverInitError::Retry)?
        } else {
            return Err(DriverInitError::DeviceTreeError);
        }

        register_mailbox(self.clone(), self.buffer.clone())
            .map_err(|_| DriverInitError::DeviceTreeError)?;

        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl InterruptHandler for MailboxDevice {
    fn handle_irq(&self, _virq: u32) {
        loop {
            let status = self.get_mbox_0_status();
            if (status & MBOX_STATUS_EMPTY) != 0 {
                break;
            }

            let msg = self.read_mbox_0();
            self.buffer.lock().push_back(msg);
        }
    }
}

impl Mailbox for MailboxDevice {
    type Message = u32;

    fn enable(&self) {
        self.enabled.store(true, Ordering::Release);
        self.enable_interrupts();
    }

    fn disable(&self) {
        self.disable_interrupts();
        self.enabled.store(false, Ordering::Release);
    }

    fn send(&self, data: u32) -> MailboxResult<()> {
        if !self.enabled.load(Ordering::Acquire) {
            return Err(MailboxError::MailboxDisabled);
        }

        let lock = self.lock.lock_irq();
        self.write_mbox_1(data);
        drop(lock);

        Ok(())
    }

    fn ready(&self) -> bool {
        if !self.enabled.load(Ordering::Acquire) {
            return false;
        }

        let lock = self.lock.lock_irq();
        let status = self.get_mbox_1_status();
        drop(lock);

        (status & MBOX_STATUS_FULL) == 0
    }
}

pub struct MailboxDriver {
    dev_registry: DriverRegistry<MailboxDevice>,
}

impl MailboxDriver {
    const fn new() -> Self {
        Self {
            dev_registry: DriverRegistry::new(),
        }
    }
}

impl PlatformDriver for MailboxDriver {
    fn compatible(&self) -> &[&str] {
        &["brcm,bcm2835-mbox"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        let reg = node.reg().ok_or(DriverInitError::DeviceTreeError)?;
        if reg.len() != 1 {
            return Err(DriverInitError::DeviceTreeError);
        }

        let (phys_addr, length) = node
            .resolve_phys_address_and_length(0)
            .ok_or(DriverInitError::DeviceTreeError)?;
        let addr = map_io_region(phys_addr, length);

        let dev = MailboxDevice::new(node.path(), addr);
        let dev = Arc::new(dev);

        dev.clone().global_setup(node)?;

        self.dev_registry.add_device(node.path(), dev.clone());

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

pub static DRIVER: MailboxDriver = MailboxDriver::new();
