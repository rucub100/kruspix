// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! See [Mailbox property interface](https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::arch::cpu::{clean_data_cache_range, invalidate_data_cache_range};
use crate::drivers::mailbox::{MailboxClient, take_mailbox};
use crate::drivers::{DEVICE_MANAGER, Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::prop::PropertyValue;
use crate::kernel::devicetree::{PHandle, get_devicetree};
use crate::kernel::sync::{OnceLock, SpinLock};
use crate::kprintln;
use crate::mm::virt_to_phys;

const MBOX_0_CH_8_ARM_VC_TAGS: u32 = 8;

pub const REQUEST_CODE: u32 = 0x00000000;
const RESPONSE_CODE_SUCCESS: u32 = 0x80000000;
const RESPONSE_CODE_ERROR: u32 = 0x80000001;

pub mod tag {
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
    pub const CLOCK_GET_CLOCK_STATE: u32 = 0x00030001;
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

pub struct TagIdentifier(u32);

#[repr(C, align(4))]
pub struct Tag<const WORDS: usize> {
    identifier: TagIdentifier,
    value_buffer_size: u32,
    req_res_code: u32,
    value_buffer: [u32; WORDS],
}

impl<const WORDS: usize> Tag<WORDS> {
    const fn new(identifier: TagIdentifier) -> Self {
        Self {
            identifier,
            value_buffer_size: (WORDS * size_of::<u32>()) as u32,
            req_res_code: REQUEST_CODE,
            value_buffer: [0; WORDS],
        }
    }
}

impl Tag<0> {
    const fn release_buffer() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_RELEASE_BUFFER))
    }
}

impl Tag<1> {
    const fn get_firmware_revision() -> Self {
        Self::new(TagIdentifier(tag::VIDEO_CORE_GET_FIRMWARE_REVISION))
    }

    const fn get_board_model() -> Self {
        Self::new(TagIdentifier(tag::HARDWARE_GET_BOARD_MODEL))
    }

    const fn get_board_revision() -> Self {
        Self::new(TagIdentifier(tag::HARDWARE_GET_BOARD_REVISION))
    }

    const fn get_dma_channels() -> Self {
        Self::new(TagIdentifier(tag::SHARED_GET_DMA_CHANNELS))
    }

    const fn lock_memory() -> Self {
        Self::new(TagIdentifier(tag::MEMORY_LOCK_MEMORY))
    }

    const fn unlock_memory() -> Self {
        Self::new(TagIdentifier(tag::MEMORY_UNLOCK_MEMORY))
    }

    const fn release_memory() -> Self {
        Self::new(TagIdentifier(tag::MEMORY_RELEASE_MEMORY))
    }

    const fn blank_screen() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_BLANK_SCREEN))
    }

    const fn get_depth() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_GET_DEPTH))
    }

    const fn test_depth() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_TEST_DEPTH))
    }

    const fn set_depth() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_DEPTH))
    }

    const fn get_pixel_order() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_GET_PIXEL_ORDER))
    }

    const fn test_pixel_order() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_TEST_PIXEL_ORDER))
    }

    const fn set_pixel_order() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_PIXEL_ORDER))
    }

    const fn get_alpha_mode() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_GET_ALPHA_MODE))
    }

    const fn test_alpha_mode() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_TEST_ALPHA_MODE))
    }

    const fn set_alpha_mode() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_ALPHA_MODE))
    }

    const fn get_pitch() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_GET_PITCH))
    }
}

impl Tag<2> {
    const fn get_board_mac_address() -> Self {
        Self::new(TagIdentifier(tag::HARDWARE_GET_BOARD_MAC_ADDRESS))
    }

    const fn get_board_serial() -> Self {
        Self::new(TagIdentifier(tag::HARDWARE_GET_BOARD_SERIAL))
    }

    const fn get_arm_memory() -> Self {
        Self::new(TagIdentifier(tag::HARDWARE_GET_ARM_MEMORY))
    }

    const fn get_vc_memory() -> Self {
        Self::new(TagIdentifier(tag::HARDWARE_GET_VC_MEMORY))
    }

    const fn get_power_state() -> Self {
        Self::new(TagIdentifier(tag::POWER_GET_POWER_STATE))
    }

    const fn get_timing() -> Self {
        Self::new(TagIdentifier(tag::POWER_GET_TIMING))
    }

    const fn set_power_state() -> Self {
        Self::new(TagIdentifier(tag::POWER_SET_POWER_STATE))
    }

    const fn get_clock_state() -> Self {
        Self::new(TagIdentifier(tag::CLOCK_GET_CLOCK_STATE))
    }

    const fn set_clock_state() -> Self {
        Self::new(TagIdentifier(tag::CLOCK_SET_CLOCK_STATE))
    }

    const fn get_clock_rate() -> Self {
        Self::new(TagIdentifier(tag::CLOCK_GET_CLOCK_RATE))
    }

    const fn get_onboard_led_status() -> Self {
        Self::new(TagIdentifier(tag::LED_GET_ONBOARD_LED_STATUS))
    }

    const fn test_onboard_led_status() -> Self {
        Self::new(TagIdentifier(tag::LED_TEST_ONBOARD_LED_STATUS))
    }

    const fn set_onboard_led_status() -> Self {
        Self::new(TagIdentifier(tag::LED_SET_ONBOARD_LED_STATUS))
    }

    const fn get_clock_rate_measured() -> Self {
        Self::new(TagIdentifier(tag::CLOCK_GET_CLOCK_RATE_MEASURED))
    }

    const fn get_max_clock_rate() -> Self {
        Self::new(TagIdentifier(tag::CLOCK_GET_MAX_CLOCK_RATE))
    }

    const fn get_min_clock_rate() -> Self {
        Self::new(TagIdentifier(tag::CLOCK_GET_MIN_CLOCK_RATE))
    }

    const fn get_turbo() -> Self {
        Self::new(TagIdentifier(tag::CLOCK_GET_TURBO))
    }

    const fn set_turbo() -> Self {
        Self::new(TagIdentifier(tag::CLOCK_SET_TURBO))
    }

    const fn get_voltage() -> Self {
        Self::new(TagIdentifier(tag::VOLTAGE_GET_VOLTAGE))
    }

    const fn set_voltage() -> Self {
        Self::new(TagIdentifier(tag::VOLTAGE_SET_VOLTAGE))
    }

    const fn get_max_voltage() -> Self {
        Self::new(TagIdentifier(tag::VOLTAGE_GET_MAX_VOLTAGE))
    }

    const fn get_min_voltage() -> Self {
        Self::new(TagIdentifier(tag::VOLTAGE_GET_MIN_VOLTAGE))
    }

    const fn get_temperature() -> Self {
        Self::new(TagIdentifier(tag::TEMPERATURE_GET_TEMPERATURE))
    }

    const fn get_max_temperature() -> Self {
        Self::new(TagIdentifier(tag::TEMPERATURE_GET_MAX_TEMPERATURE))
    }

    const fn get_dispmanx_resource_mem_handle() -> Self {
        Self::new(TagIdentifier(tag::MEMORY_GET_DISPMANX_RESOURCE_MEM_HANDLE))
    }

    const fn allocate_buffer() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_ALLOCATE_BUFFER))
    }

    const fn get_physical_width_height() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_GET_PHYSICAL_WIDTH_HEIGHT))
    }

    const fn test_physical_width_height() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_TEST_PHYSICAL_WIDTH_HEIGHT))
    }

    const fn set_physical_width_height() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_PHYSICAL_WIDTH_HEIGHT))
    }

    const fn get_virtual_width_height() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_GET_VIRTUAL_WIDTH_HEIGHT))
    }

    const fn test_virtual_width_height() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_TEST_VIRTUAL_WIDTH_HEIGHT))
    }

    const fn set_virtual_width_height() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_VIRTUAL_WIDTH_HEIGHT))
    }

    const fn get_virtual_offset() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_GET_VIRTUAL_OFFSET))
    }

    const fn test_virtual_offset() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_TEST_VIRTUAL_OFFSET))
    }

    const fn set_virtual_offset() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_VIRTUAL_OFFSET))
    }
}

impl Tag<3> {
    const fn set_clock_rate() -> Self {
        Self::new(TagIdentifier(tag::CLOCK_SET_CLOCK_RATE))
    }

    const fn allocate_memory() -> Self {
        Self::new(TagIdentifier(tag::MEMORY_ALLOCATE_MEMORY))
    }
}

impl Tag<4> {
    const fn get_overscan() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_GET_OVERSCAN))
    }

    const fn test_overscan() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_TEST_OVERSCAN))
    }

    const fn set_overscan() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_OVERSCAN))
    }

    const fn set_cursor_state() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_CURSOR_STATE))
    }
}

impl Tag<6> {
    const fn set_cursor_info() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_CURSOR_INFO))
    }
}

impl Tag<7> {
    const fn execute_code() -> Self {
        Self::new(TagIdentifier(tag::MEMORY_EXECUTE_CODE))
    }
}

impl Tag<32> {
    const fn get_clocks() -> Self {
        Self::new(TagIdentifier(tag::HARDWARE_GET_CLOCKS))
    }
}

impl Tag<34> {
    const fn get_edid_block() -> Self {
        Self::new(TagIdentifier(tag::MEMORY_GET_EDID_BLOCK))
    }
}

impl Tag<256> {
    /// Limit the command line buffer to 1KiB.
    const fn get_command_line() -> Self {
        Self::new(TagIdentifier(tag::CONFIG_GET_COMMAND_LINE))
    }

    const fn get_palette() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_GET_PALETTE))
    }
}

impl Tag<258> {
    const fn test_palette() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_TEST_PALETTE))
    }

    const fn set_palette() -> Self {
        Self::new(TagIdentifier(tag::FRAMEBUFFER_SET_PALETTE))
    }
}

struct ConstChecks<const N: usize, const WORDS: usize>;

impl<const N: usize, const WORDS: usize> ConstChecks<N, WORDS> {
    const OK: () = {
        assert!(
            N <= WORDS - 4,
            "Tag payload is too large for the allocated Message size!"
        );
        assert!(
            (WORDS + 2) % 16 == 0,
            "Message size must be a multiple of 64 bytes to prevent cache line tearing!"
        );
    };
}

/// The message structure for communicating with the firmware via the mailbox property interface.
/// The total size of the message must be a multiple of 64 bytes to avoid cache line issues.
#[repr(C, align(64))]
pub struct Message<const WORDS: usize> {
    size: u32,
    req_res_code: u32,
    data: [u32; WORDS],
}

impl<const WORDS: usize> Message<WORDS> {
    #[inline]
    const fn new<const N: usize>(tag: Tag<N>) -> Self {
        let _ = ConstChecks::<N, WORDS>::OK;

        let mut data = [0u32; WORDS];

        data[0] = tag.identifier.0;
        data[1] = tag.value_buffer_size;
        data[2] = tag.req_res_code;

        let mut i = 0;
        while i < tag.value_buffer.len() {
            data[3 + i] = tag.value_buffer[i];
            i += 1;
        }

        data[3 + tag.value_buffer.len()] = tag::END_TAG;

        Self {
            size: ((2 + WORDS) * size_of::<u32>()) as u32,
            req_res_code: REQUEST_CODE,
            data,
        }
    }
}

impl Message<14> {
    const fn new_get_firmware_revision() -> Self {
        Self::new(Tag::get_firmware_revision())
    }

    const fn new_get_board_model() -> Self {
        Self::new(Tag::get_board_model())
    }

    const fn new_get_board_revision() -> Self {
        Self::new(Tag::get_board_revision())
    }

    const fn new_get_board_mac_address() -> Self {
        Self::new(Tag::get_board_mac_address())
    }

    const fn new_get_board_serial() -> Self {
        Self::new(Tag::get_board_serial())
    }

    const fn new_get_arm_memory() -> Self {
        Self::new(Tag::get_arm_memory())
    }

    const fn new_get_vc_memory() -> Self {
        Self::new(Tag::get_vc_memory())
    }

    const fn new_get_dma_channels() -> Self {
        Self::new(Tag::get_dma_channels())
    }

    const fn new_get_onboard_led_status() -> Self {
        Self::new(Tag::get_onboard_led_status())
    }

    const fn new_test_onboard_led_status() -> Self {
        Self::new(Tag::test_onboard_led_status())
    }

    const fn new_set_onboard_led_status(pin_number: u32, status: u32) -> Self {
        let mut tag = Tag::set_onboard_led_status();
        tag.value_buffer[0] = pin_number;
        tag.value_buffer[1] = status;
        Self::new(tag)
    }

    const fn new_allocate_buffer(alignment: u32) -> Self {
        let mut tag = Tag::allocate_buffer();
        tag.value_buffer[0] = alignment;
        Self::new(tag)
    }

    const fn new_release_buffer() -> Self {
        Self::new(Tag::release_buffer())
    }

    const fn new_blank_screen(state: u32) -> Self {
        let mut tag = Tag::blank_screen();
        tag.value_buffer[0] = state;
        Self::new(tag)
    }

    const fn new_get_physical_width_height() -> Self {
        Self::new(Tag::get_physical_width_height())
    }

    const fn new_test_physical_width_height(width: u32, height: u32) -> Self {
        let mut tag = Tag::test_physical_width_height();
        tag.value_buffer[0] = width;
        tag.value_buffer[1] = height;
        Self::new(tag)
    }

    const fn new_set_physical_width_height(width: u32, height: u32) -> Self {
        let mut tag = Tag::set_physical_width_height();
        tag.value_buffer[0] = width;
        tag.value_buffer[1] = height;
        Self::new(tag)
    }

    const fn new_get_virtual_width_height() -> Self {
        Self::new(Tag::get_virtual_width_height())
    }

    const fn new_test_virtual_width_height(width: u32, height: u32) -> Self {
        let mut tag = Tag::test_virtual_width_height();
        tag.value_buffer[0] = width;
        tag.value_buffer[1] = height;
        Self::new(tag)
    }

    const fn new_set_virtual_width_height(width: u32, height: u32) -> Self {
        let mut tag = Tag::set_virtual_width_height();
        tag.value_buffer[0] = width;
        tag.value_buffer[1] = height;
        Self::new(tag)
    }

    const fn new_get_depth() -> Self {
        Self::new(Tag::get_depth())
    }

    const fn new_test_depth(bits_per_pixel: u32) -> Self {
        let mut tag = Tag::test_depth();
        tag.value_buffer[0] = bits_per_pixel;
        Self::new(tag)
    }

    const fn new_set_depth(bits_per_pixel: u32) -> Self {
        let mut tag = Tag::set_depth();
        tag.value_buffer[0] = bits_per_pixel;
        Self::new(tag)
    }

    const fn new_get_pixel_order() -> Self {
        Self::new(Tag::get_pixel_order())
    }

    const fn new_test_pixel_order(state: u32) -> Self {
        let mut tag = Tag::test_pixel_order();
        tag.value_buffer[0] = state;
        Self::new(tag)
    }

    const fn new_set_pixel_order(state: u32) -> Self {
        let mut tag = Tag::set_pixel_order();
        tag.value_buffer[0] = state;
        Self::new(tag)
    }

    const fn new_get_alpha_mode() -> Self {
        Self::new(Tag::get_alpha_mode())
    }

    const fn new_test_alpha_mode(state: u32) -> Self {
        let mut tag = Tag::test_alpha_mode();
        tag.value_buffer[0] = state;
        Self::new(tag)
    }

    const fn new_set_alpha_mode(state: u32) -> Self {
        let mut tag = Tag::set_alpha_mode();
        tag.value_buffer[0] = state;
        Self::new(tag)
    }

    const fn new_get_pitch() -> Self {
        Self::new(Tag::get_pitch())
    }

    const fn new_get_virtual_offset() -> Self {
        Self::new(Tag::get_virtual_offset())
    }

    const fn new_test_virtual_offset(x: u32, y: u32) -> Self {
        let mut tag = Tag::test_virtual_offset();
        tag.value_buffer[0] = x;
        tag.value_buffer[1] = y;
        Self::new(tag)
    }

    const fn new_set_virtual_offset(x: u32, y: u32) -> Self {
        let mut tag = Tag::set_virtual_offset();
        tag.value_buffer[0] = x;
        tag.value_buffer[1] = y;
        Self::new(tag)
    }

    const fn new_get_overscan() -> Self {
        Self::new(Tag::get_overscan())
    }

    const fn new_test_overscan(top: u32, bottom: u32, left: u32, right: u32) -> Self {
        let mut tag = Tag::test_overscan();
        tag.value_buffer[0] = top;
        tag.value_buffer[1] = bottom;
        tag.value_buffer[2] = left;
        tag.value_buffer[3] = right;
        Self::new(tag)
    }

    const fn new_set_overscan(top: u32, bottom: u32, left: u32, right: u32) -> Self {
        let mut tag = Tag::set_overscan();
        tag.value_buffer[0] = top;
        tag.value_buffer[1] = bottom;
        tag.value_buffer[2] = left;
        tag.value_buffer[3] = right;
        Self::new(tag)
    }

    const fn new_get_power_state(device_id: u32) -> Self {
        let mut tag = Tag::get_power_state();
        tag.value_buffer[0] = device_id;
        Self::new(tag)
    }

    const fn new_get_timing(device_id: u32) -> Self {
        let mut tag = Tag::get_timing();
        tag.value_buffer[0] = device_id;
        Self::new(tag)
    }

    const fn new_set_power_state(device_id: u32, state: u32) -> Self {
        let mut tag = Tag::set_power_state();
        tag.value_buffer[0] = device_id;
        tag.value_buffer[1] = state;
        Self::new(tag)
    }

    const fn new_get_clock_state(clock_id: u32) -> Self {
        let mut tag = Tag::get_clock_state();
        tag.value_buffer[0] = clock_id;
        Self::new(tag)
    }

    const fn new_set_clock_state(clock_id: u32, state: u32) -> Self {
        let mut tag = Tag::set_clock_state();
        tag.value_buffer[0] = clock_id;
        tag.value_buffer[1] = state;
        Self::new(tag)
    }

    const fn new_get_clock_rate(clock_id: u32) -> Self {
        let mut tag = Tag::get_clock_rate();
        tag.value_buffer[0] = clock_id;
        Self::new(tag)
    }

    const fn new_get_clock_rate_measured(clock_id: u32) -> Self {
        let mut tag = Tag::get_clock_rate_measured();
        tag.value_buffer[0] = clock_id;
        Self::new(tag)
    }

    const fn new_set_clock_rate(clock_id: u32, rate: u32, skip_setting_turbo: u32) -> Self {
        let mut tag = Tag::set_clock_rate();
        tag.value_buffer[0] = clock_id;
        tag.value_buffer[1] = rate;
        tag.value_buffer[2] = skip_setting_turbo;
        Self::new(tag)
    }

    const fn new_get_max_clock_rate(clock_id: u32) -> Self {
        let mut tag = Tag::get_max_clock_rate();
        tag.value_buffer[0] = clock_id;
        Self::new(tag)
    }

    const fn new_get_min_clock_rate(clock_id: u32) -> Self {
        let mut tag = Tag::get_min_clock_rate();
        tag.value_buffer[0] = clock_id;
        Self::new(tag)
    }

    const fn new_get_turbo(id: u32) -> Self {
        let mut tag = Tag::get_turbo();
        tag.value_buffer[0] = id;
        Self::new(tag)
    }

    const fn new_set_turbo(id: u32, level: u32) -> Self {
        let mut tag = Tag::set_turbo();
        tag.value_buffer[0] = id;
        tag.value_buffer[1] = level;
        Self::new(tag)
    }

    const fn new_get_voltage(voltage_id: u32) -> Self {
        let mut tag = Tag::get_voltage();
        tag.value_buffer[0] = voltage_id;
        Self::new(tag)
    }

    const fn new_set_voltage(voltage_id: u32, value: u32) -> Self {
        let mut tag = Tag::set_voltage();
        tag.value_buffer[0] = voltage_id;
        tag.value_buffer[1] = value;
        Self::new(tag)
    }

    const fn new_get_max_voltage(voltage_id: u32) -> Self {
        let mut tag = Tag::get_max_voltage();
        tag.value_buffer[0] = voltage_id;
        Self::new(tag)
    }

    const fn new_get_min_voltage(voltage_id: u32) -> Self {
        let mut tag = Tag::get_min_voltage();
        tag.value_buffer[0] = voltage_id;
        Self::new(tag)
    }

    const fn new_get_temperature(temperature_id: u32) -> Self {
        let mut tag = Tag::get_temperature();
        tag.value_buffer[0] = temperature_id;
        Self::new(tag)
    }

    const fn new_get_max_temperature(temperature_id: u32) -> Self {
        let mut tag = Tag::get_max_temperature();
        tag.value_buffer[0] = temperature_id;
        Self::new(tag)
    }

    const fn new_allocate_memory(size: u32, alignment: u32, flags: u32) -> Self {
        let mut tag = Tag::allocate_memory();
        tag.value_buffer[0] = size;
        tag.value_buffer[1] = alignment;
        tag.value_buffer[2] = flags;
        Self::new(tag)
    }

    const fn new_lock_memory(handle: u32) -> Self {
        let mut tag = Tag::lock_memory();
        tag.value_buffer[0] = handle;
        Self::new(tag)
    }

    const fn new_unlock_memory(handle: u32) -> Self {
        let mut tag = Tag::unlock_memory();
        tag.value_buffer[0] = handle;
        Self::new(tag)
    }

    const fn new_release_memory(handle: u32) -> Self {
        let mut tag = Tag::release_memory();
        tag.value_buffer[0] = handle;
        Self::new(tag)
    }

    const fn new_execute_code(
        entry_point: u32,
        r0: u32,
        r1: u32,
        r2: u32,
        r3: u32,
        r4: u32,
        r5: u32,
    ) -> Self {
        let mut tag = Tag::execute_code();
        tag.value_buffer[0] = entry_point;
        tag.value_buffer[1] = r0;
        tag.value_buffer[2] = r1;
        tag.value_buffer[3] = r2;
        tag.value_buffer[4] = r3;
        tag.value_buffer[5] = r4;
        tag.value_buffer[6] = r5;
        Self::new(tag)
    }

    const fn new_get_dispmanx_resource_mem_handle(handle: u32) -> Self {
        let mut tag = Tag::get_dispmanx_resource_mem_handle();
        tag.value_buffer[0] = handle;
        Self::new(tag)
    }

    const fn new_set_cursor_info(
        width: u32,
        height: u32,
        pointer_to_pixels: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Self {
        let mut tag = Tag::set_cursor_info();
        tag.value_buffer[0] = width;
        tag.value_buffer[1] = height;
        tag.value_buffer[2] = 0; // unused
        tag.value_buffer[3] = pointer_to_pixels;
        tag.value_buffer[4] = hotspot_x;
        tag.value_buffer[5] = hotspot_y;
        Self::new(tag)
    }

    const fn new_set_cursor_state(enable: u32, x: u32, y: u32, flags: u32) -> Self {
        let mut tag = Tag::set_cursor_state();
        tag.value_buffer[0] = enable;
        tag.value_buffer[1] = x;
        tag.value_buffer[2] = y;
        tag.value_buffer[3] = flags;
        Self::new(tag)
    }
}

impl Message<46> {
    const fn new_get_clocks() -> Self {
        Self::new(Tag::get_clocks())
    }

    const fn new_get_edid_block(block_id: u32) -> Self {
        let mut tag = Tag::get_edid_block();
        tag.value_buffer[0] = block_id;
        Self::new(tag)
    }
}

impl Message<318> {
    const fn new_get_palette() -> Self {
        Self::new(Tag::get_palette())
    }

    const fn palette(offset: u32, length: u32, palette: &[u32], mut tag: Tag<258>) -> Self {
        tag.value_buffer[0] = offset;
        tag.value_buffer[1] = length;
        let mut i = 0;

        let len = if palette.len() < tag.value_buffer.len() - 2 {
            palette.len()
        } else {
            tag.value_buffer.len() - 2
        };

        while i < len {
            tag.value_buffer[2 + i] = palette[i];
            i += 1;
        }

        Self::new(tag)
    }

    const fn new_test_palette(offset: u32, length: u32, palette: &[u32]) -> Self {
        let tag = Tag::test_palette();
        Self::palette(offset, length, palette, tag)
    }

    const fn new_set_palette(offset: u32, length: u32, palette: &[u32]) -> Self {
        let tag = Tag::set_palette();
        Self::palette(offset, length, palette, tag)
    }

    const fn new_get_command_line() -> Self {
        Self::new(Tag::get_command_line())
    }
}

pub struct RpiFirmware {
    id: String,
    mbox: OnceLock<&'static MailboxClient>,
    lock: SpinLock<()>,
}

impl RpiFirmware {
    const fn new(id: String) -> Self {
        Self {
            id,
            mbox: OnceLock::new(),
            lock: SpinLock::new(()),
        }
    }

    pub fn property<const N: usize>(&self, msg: &mut Message<N>) -> Result<(), ()> {
        let mbox = self.mbox.get().expect("Mailbox not initialized");

        let _guard = self.lock.lock();

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

    pub fn get_firmware_revision(&self) -> Result<u32, ()> {
        let mut msg = Message::new_get_firmware_revision();
        self.property(&mut msg)?;
        Ok(msg.data[3])
    }

    pub fn get_board_model(&self) -> Result<u32, ()> {
        let mut msg = Message::new_get_board_model();
        self.property(&mut msg)?;
        Ok(msg.data[3])
    }

    pub fn get_board_revision(&self) -> Result<u32, ()> {
        let mut msg = Message::new_get_board_revision();
        self.property(&mut msg)?;
        Ok(msg.data[3])
    }

    pub fn get_board_mac_address(&self) -> Result<[u8; 6], ()> {
        let mut msg = Message::new_get_board_mac_address();
        self.property(&mut msg)?;

        let word0 = msg.data[3].to_ne_bytes();
        let word1 = msg.data[4].to_ne_bytes();

        Ok([word0[0], word0[1], word0[2], word0[3], word1[0], word1[1]])
    }

    pub fn get_board_serial(&self) -> Result<u64, ()> {
        let mut msg = Message::new_get_board_serial();
        self.property(&mut msg)?;

        let low = msg.data[3] as u64;
        let high = msg.data[4] as u64;

        Ok((high << 32) | low)
    }

    pub fn get_arm_memory(&self) -> Result<(u32, u32), ()> {
        let mut msg = Message::new_get_arm_memory();
        self.property(&mut msg)?;

        // base address and size in bytes
        Ok((msg.data[3], msg.data[4]))
    }

    pub fn get_vc_memory(&self) -> Result<(u32, u32), ()> {
        let mut msg = Message::new_get_vc_memory();
        self.property(&mut msg)?;

        // base address and size in bytes
        Ok((msg.data[3], msg.data[4]))
    }

    pub fn get_command_line(&self) -> Result<String, ()> {
        let mut msg = Message::new_get_command_line();
        self.property(&mut msg)?;

        let mut bytes = Vec::with_capacity(msg.data.len());

        for word in &msg.data[3..] {
            let chunk = word.to_ne_bytes();
            for &b in &chunk {
                if b == 0 {
                    return String::from_utf8(bytes).map_err(|_| ());
                }
                bytes.push(b);
            }
        }

        String::from_utf8(bytes).map_err(|_| ())
    }

    pub fn get_dma_channels(&self) -> Result<u32, ()> {
        let mut msg = Message::new_get_dma_channels();
        self.property(&mut msg)?;

        // bitmask of usable channels (0-15)
        Ok(msg.data[3])
    }

    fn print_info(&self) -> Result<(), ()> {
        let firmware_revision = self.get_firmware_revision()?;
        kprintln!("Firmware revision: {:#x}", firmware_revision);

        let board_model = self.get_board_model()?;
        kprintln!("Board model: {:#x}", board_model);

        let board_revision = self.get_board_revision()?;
        kprintln!("Board revision: {:#x}", board_revision);

        let board_serial = self.get_board_serial()?;
        kprintln!("Board serial: {:#x}", board_serial);

        let board_mac_address = self.get_board_mac_address()?;
        kprintln!(
            "Board MAC address: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            board_mac_address[0],
            board_mac_address[1],
            board_mac_address[2],
            board_mac_address[3],
            board_mac_address[4],
            board_mac_address[5]
        );

        let arm_memory = self.get_arm_memory()?;
        kprintln!(
            "ARM memory: base address = {:#x}, size = {} MiB",
            arm_memory.0,
            arm_memory.1 / (1024 * 1024)
        );

        let vc_memory = self.get_vc_memory()?;
        kprintln!(
            "VC memory: base address = {:#x}, size = {} MiB",
            vc_memory.0,
            vc_memory.1 / (1024 * 1024)
        );

        let command_line = self.get_command_line()?;
        kprintln!("Command line: {}", command_line);

        let dma_channels = self.get_dma_channels()?;
        kprintln!("Usable DMA channels bitmask (0-15): {:016b}", dma_channels);

        // power
        // clocks
        // LED status
        // voltage
        // temperature
        // display info
        //

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

        RPI_FIRMWARE
            .set(dev.clone())
            .map_err(|_| DriverInitError::DeviceFailed)?;

        self.dev_registry.add_device(node.path(), dev.clone());

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

pub static DRIVER: RpiFirmwareDriver = RpiFirmwareDriver::new();

static RPI_FIRMWARE: OnceLock<Arc<RpiFirmware>> = OnceLock::new();
pub fn get_firmware() -> Option<Arc<RpiFirmware>> {
    RPI_FIRMWARE.get().cloned()
}
