// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! Documentation for the Broadcom BCM2835 VideoCore mailbox IPC can be found on the
//! [Mailboxes Wiki](https://github.com/raspberrypi/firmware/wiki/Mailboxes).

use alloc::string::String;
use alloc::sync::Arc;

use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::std_prop::StandardProperties;
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
}

impl MailboxDevice {
    const fn new(id: String, reg_base: usize) -> Self {
        Self { id, reg_base }
    }
}

impl Device for MailboxDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        // TODO: interrupts
        todo!()
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
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
