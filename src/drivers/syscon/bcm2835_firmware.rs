// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! See [Mailbox property interface](https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface).

use crate::arch::cpu::{clean_data_cache_range, invalidate_data_cache_range};
use crate::drivers::mailbox::{MailboxClient, take_mailbox};
use crate::drivers::{DEVICE_MANAGER, Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::prop::PropertyValue;
use crate::kernel::devicetree::{PHandle, get_devicetree};
use crate::kernel::sync::OnceLock;
use crate::kprintln;
use crate::mm::virt_to_phys;
use alloc::string::String;
use alloc::sync::Arc;

const MBOX_0_CH_8_ARM_VC_TAGS: u32 = 8;

const REQUEST_CODE: u32 = 0x00000000;
const RESPONSE_CODE_SUCCESS: u32 = 0x80000000;
const RESPONSE_CODE_ERROR: u32 = 0x80000001;

mod tag {
    pub const END_TAG: u32 = 0x00000000;
    // VideoCore
    pub const VIDEO_CORE_GET_FIRMWARE_REVISION: u32 = 0x00000001;
    // Hardware
    pub const HARDWARE_GET_BOARD_MODEL: u32 = 0x00010001;
    pub const HARDWARE_GET_BOARD_REVISION: u32 = 0x00010002;
    pub const HARDWARE_GET_BOARD_MAC_ADDRESS: u32 = 0x00010003;
    pub const HARDWARE_GET_BOARD_SERIAL: u32 = 0x00010004;
    pub const HARDWARE_GET_ARM_MEMORY: u32 = 0x00010005;
    pub const HARDWARE_GET_VC_MEMORY: u32 = 0x00010006;
    pub const HARDWARE_GET_CLOCKS: u32 = 0x00010007;
    // Config
    pub const CONFIG_GET_COMMAND_LINE: u32 = 0x00050001;
    // Shared resource management
    pub const SHARED_GET_DMA_CHANNELS: u32 = 0x00060001;
    // Power
    pub mod power {
        pub const SD_CARD: u32 = 0x00000000;
        pub const UART0: u32 = 0x00000001;
        pub const UART1: u32 = 0x00000002;
        pub const USB_HCD: u32 = 0x00000003;
        pub const I2C0: u32 = 0x00000004;
        pub const I2C1: u32 = 0x00000005;
        pub const I2C2: u32 = 0x00000006;
        pub const SPI: u32 = 0x00000007;
        pub const CCP2TX: u32 = 0x00000008;
    }
    pub const POWER_GET_POWER_STATE: u32 = 0x00020001;
    pub const POWER_GET_TIMING: u32 = 0x00020002;
    pub const POWER_SET_POWER_STATE: u32 = 0x00028001;
    // Clocks
    pub mod clocks {
        pub const EMMC: u32 = 0x00000001;
        pub const UART: u32 = 0x00000002;
        pub const ARM: u32 = 0x00000003;
        pub const CORE: u32 = 0x00000004;
        pub const V3D: u32 = 0x00000005;
        pub const H264: u32 = 0x00000006;
        pub const ISP: u32 = 0x00000007;
        pub const SDRAM: u32 = 0x00000008;
        pub const PIXEL: u32 = 0x00000009;
        pub const PWM: u32 = 0x0000000a;
        pub const HEVC: u32 = 0x0000000b;
        pub const EMMC2: u32 = 0x0000000c;
        pub const M2MC: u32 = 0x0000000d;
        pub const PIXEL_BVB: u32 = 0x0000000e;
    }
    pub const CLOCK_GET_CLOCKS: u32 = 0x00030001;
    pub const CLOCK_SET_CLOCK_STATE: u32 = 0x00038001;
    pub const CLOCK_GET_CLOCK_RATE: u32 = 0x00030002;
    pub const CLOCK_GET_CLOCK_RATE_MEASURED: u32 = 0x00030047;
    pub const CLOCK_SET_CLOCK_RATE: u32 = 0x00038002;
    pub const CLOCK_GET_MAX_CLOCK_RATE: u32 = 0x00030004;
    pub const CLOCK_GET_MIN_CLOCK_RATE: u32 = 0x00030007;
    pub const CLOCK_GET_TURBO: u32 = 0x00030009;
    pub const CLOCK_SET_TURBO: u32 = 0x00038009;
    // LEDs
    pub const LED_GET_ONBOARD_LED_STATUS: u32 = 0x00030041;
    pub const LED_TEST_ONBOARD_LED_STATUS: u32 = 0x00034041;
    pub const LED_SET_ONBOARD_LED_STATUS: u32 = 0x00038041;
    // Voltage
    pub mod voltage {
        pub const CORE: u32 = 0x000000001;
        pub const SDRAM_C: u32 = 0x000000002;
        pub const SDRAM_P: u32 = 0x000000003;
        pub const SDRAM_I: u32 = 0x000000004;
    }
    pub const VOLTAGE_GET_VOLTAGE: u32 = 0x00030003;
    pub const VOLTAGE_SET_VOLTAGE: u32 = 0x00038003;
    pub const VOLTAGE_GET_MAX_VOLTAGE: u32 = 0x00030005;
    pub const VOLTAGE_GET_MIN_VOLTAGE: u32 = 0x00030008;
    // Temperature
    pub const TEMPERATURE_GET_TEMPERATURE: u32 = 0x00030006;
    pub const TEMPERATURE_GET_MAX_TEMPERATURE: u32 = 0x0003000a;
    // Memory management
    pub mod memory {
        // can be resized to 0 at any time; use for cached data
        pub const MEM_FLAG_DISCARDABLE: u32 = 1 << 0;

        // normal allocating alias; don't use from ARM
        pub const MEM_FLAG_NORMAL: u32 = 0 << 2;

        // 0xc alias uncached
        pub const MEM_FLAG_DIRECT: u32 = 1 << 2;

        // 0x8 alias; non-allocating in L2 but coherent
        pub const MEM_FLAG_COHERENT: u32 = 2 << 2;

        // allocating in L2
        pub const MEM_FLAG_L1_NONALLOCATING: u32 = MEM_FLAG_DIRECT | MEM_FLAG_COHERENT;

        // initialize buffer to all zeros
        pub const MEM_FLAG_ZERO: u32 = 1 << 4;

        // don't initialize (default is initialize to all ones)
        pub const MEM_FLAG_NO_INIT: u32 = 1 << 5;

        // likely to be locked for long periods of time
        pub const MEM_FLAG_HINT_PERMALOCK: u32 = 1 << 6;
    }
    pub const MEMORY_ALLOCATE_MEMORY: u32 = 0x0003000c;
    pub const MEMORY_LOCK_MEMORY: u32 = 0x0003000d;
    pub const MEMORY_UNLOCK_MEMORY: u32 = 0x0003000e;
    pub const MEMORY_RELEASE_MEMORY: u32 = 0x0003000f;
    pub const MEMORY_EXECUTE_CODE: u32 = 0x00030010;
    pub const MEMORY_GET_DISPMANX_RESOURCE_MEM_HANDLE: u32 = 0x00030014;
    pub const MEMORY_GET_EDID_BLOCK: u32 = 0x00030020;
    // Framebuffer
    pub const FRAMEBUFFER_ALLOCATE_BUFFER: u32 = 0x00040001;
    pub const FRAMEBUFFER_RELEASE_BUFFER: u32 = 0x00048001;
    pub const FRAMEBUFFER_BLANK_SCREEN: u32 = 0x00040002;
    pub const FRAMEBUFFER_GET_PHYSICAL_WIDTH_HEIGHT: u32 = 0x00040003;
    pub const FRAMEBUFFER_TEST_PHYSICAL_WIDTH_HEIGHT: u32 = 0x00044003;
    pub const FRAMEBUFFER_SET_PHYSICAL_WIDTH_HEIGHT: u32 = 0x00048003;
    pub const FRAMEBUFFER_GET_VIRTUAL_WIDTH_HEIGHT: u32 = 0x00040004;
    pub const FRAMEBUFFER_TEST_VIRTUAL_WIDTH_HEIGHT: u32 = 0x00044004;
    pub const FRAMEBUFFER_SET_VIRTUAL_WIDTH_HEIGHT: u32 = 0x00048004;
    pub const FRAMEBUFFER_GET_DEPTH: u32 = 0x00040005;
    pub const FRAMEBUFFER_TEST_DEPTH: u32 = 0x00044005;
    pub const FRAMEBUFFER_SET_DEPTH: u32 = 0x00048005;
    pub const FRAMEBUFFER_GET_PIXEL_ORDER: u32 = 0x00040006;
    pub const FRAMEBUFFER_TEST_PIXEL_ORDER: u32 = 0x00044006;
    pub const FRAMEBUFFER_SET_PIXEL_ORDER: u32 = 0x00048006;
    pub const FRAMEBUFFER_GET_ALPHA_MODE: u32 = 0x00040007;
    pub const FRAMEBUFFER_TEST_ALPHA_MODE: u32 = 0x00044007;
    pub const FRAMEBUFFER_SET_ALPHA_MODE: u32 = 0x00048007;
    pub const FRAMEBUFFER_GET_PITCH: u32 = 0x00040008;
    pub const FRAMEBUFFER_GET_VIRTUAL_OFFSET: u32 = 0x00040009;
    pub const FRAMEBUFFER_TEST_VIRTUAL_OFFSET: u32 = 0x00044009;
    pub const FRAMEBUFFER_SET_VIRTUAL_OFFSET: u32 = 0x00048009;
    pub const FRAMEBUFFER_GET_OVERSCAN: u32 = 0x0004000a;
    pub const FRAMEBUFFER_TEST_OVERSCAN: u32 = 0x0004400a;
    pub const FRAMEBUFFER_SET_OVERSCAN: u32 = 0x0004800a;
    pub const FRAMEBUFFER_GET_PALETTE: u32 = 0x0004000b;
    pub const FRAMEBUFFER_TEST_PALETTE: u32 = 0x0004400b;
    pub const FRAMEBUFFER_SET_PALETTE: u32 = 0x0004800b;
    pub const FRAMEBUFFER_SET_CURSOR_INFO: u32 = 0x00008010;
    pub const FRAMEBUFFER_SET_CURSOR_STATE: u32 = 0x00008011;
}

struct TagIdentifier(u32);

#[repr(C, align(4))]
struct Tag<const WORDS: usize> {
    identifier: TagIdentifier,
    value_buffer_size: u32,
    req_res_code: u32,
    value_buffer: [u32; WORDS],
}

impl Tag<1> {
    const fn get_firmware_revision() -> Self {
        Self {
            identifier: TagIdentifier(tag::VIDEO_CORE_GET_FIRMWARE_REVISION),
            value_buffer_size: 1 * size_of::<u32>() as u32,
            req_res_code: REQUEST_CODE,
            value_buffer: [0; 1],
        }
    }
}

/// The message structure for communicating with the firmware via the mailbox property interface.
/// The total size of the message must be a multiple of 64 bytes to avoid cache line issues.
#[repr(C, align(64))]
struct Message<const WORDS: usize> {
    size: u32,
    req_res_code: u32,
    data: [u32; WORDS],
}

impl Message<14> {
    const fn new_get_firmware_revision() -> Self {
        let tag = Tag::get_firmware_revision();

        Self {
            size: (2 + 6) * size_of::<u32>() as u32,
            req_res_code: REQUEST_CODE,
            data: [
                tag.identifier.0,
                tag.value_buffer_size,
                tag.req_res_code,
                tag.value_buffer[0],
                tag::END_TAG,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
        }
    }
}

struct RpiFirmware {
    id: String,
    mbox: OnceLock<&'static MailboxClient>,
}

impl RpiFirmware {
    const fn new(id: String) -> Self {
        Self {
            id,
            mbox: OnceLock::new(),
        }
    }

    fn property<const N: usize>(&self, msg: &mut Message<N>) -> Result<(), ()> {
        let mbox = self.mbox.get().expect("Mailbox not initialized");

        let virt_addr = msg as *const _ as usize;
        let phys_addr = virt_to_phys(virt_addr);
        let data = phys_addr as u32 | MBOX_0_CH_8_ARM_VC_TAGS;

        unsafe {
            clean_data_cache_range(virt_addr, size_of::<Message<N>>());
        }

        mbox.send(data)
            .map_err(|e| kprintln!("[ERROR] Failed to send message: {:?}", e))?;

        let result = mbox
            .receive_blocking()
            .map_err(|e| kprintln!("[ERROR] Failed to receive message: {:?}", e))?;

        if result != data {
            kprintln!(
                "[ERROR] Received response does not match sent message: expected {:#x}, got {:#x}",
                data,
                result
            );

            return Err(());
        }

        unsafe {
            invalidate_data_cache_range(virt_addr, size_of::<Message<N>>());
        }

        if msg.req_res_code != RESPONSE_CODE_SUCCESS {
            kprintln!(
                "[ERROR] Firmware rejected the property request. Code: {:#x}",
                msg.req_res_code
            );

            return Err(());
        }

        Ok(())
    }

    fn print_info(&self) -> Result<(), ()> {
        let mut msg = Message::new_get_firmware_revision();
        if self.property(&mut msg).is_ok() {
            let firmware_revision = msg.data[3];
            kprintln!("Firmware revision: {:#x}", firmware_revision);
        } else {
            kprintln!("[ERROR] Failed to get firmware revision");
            return Err(());
        }

        Ok(())
    }
}

impl Device for RpiFirmware {
    fn id(&self) -> &str {
        &self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, node: &Node) -> Result<(), DriverInitError> {
        let mbox = take_mailbox().map_err(|_| DriverInitError::DeviceFailed)?;

        self.mbox
            .set(mbox)
            .map_err(|_| DriverInitError::DeviceFailed)?;

        self.print_info()
            .map_err(|_| DriverInitError::DeviceFailed)?;

        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

pub struct RpiFirmwareDriver {
    dev_registry: DriverRegistry<RpiFirmware>,
}

impl RpiFirmwareDriver {
    pub const fn new() -> Self {
        Self {
            dev_registry: DriverRegistry::new(),
        }
    }
}

impl PlatformDriver for RpiFirmwareDriver {
    fn compatible(&self) -> &[&str] {
        &["raspberrypi,bcm2835-firmware"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        if let Some(mboxes) = node.properties().iter().find(|p| p.name() == "mboxes")
            && let Some(mboxes) = match mboxes.value() {
                PropertyValue::Unknown(prop) => prop.try_into().ok(),
                _ => None,
            }
        {
            let mboxes_phandle: PHandle = mboxes;
            let dt = get_devicetree().ok_or(DriverInitError::DeviceFailed)?;
            let mbox_node = dt
                .node_by_phandle(&mboxes_phandle)
                .ok_or(DriverInitError::DeviceFailed)?;
            // ensure the mailbox device is initialized before we initialize the firmware
            let _ = DEVICE_MANAGER
                .get_device(mbox_node.path().as_str())
                .ok_or(DriverInitError::Retry)?;
        } else {
            return Err(DriverInitError::DeviceTreeError);
        }

        let dev = RpiFirmware::new(node.path());
        let dev = Arc::new(dev);

        dev.clone().global_setup(node)?;

        self.dev_registry.add_device(node.path(), dev.clone());

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

pub static DRIVER: RpiFirmwareDriver = RpiFirmwareDriver::new();
