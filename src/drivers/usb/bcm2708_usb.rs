// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! BCM2708 USB host-controller driver for the Raspberry Pi 3.
//!
//! This driver was developed using the Linux `dwc2` host controller driver and the Circle
//! bare-metal USB host stack as technical references for register layout and initialization
//! sequencing. This Kruspix implementation is written independently for the repository's
//! MIT-licensed kernel and does not copy source code from those projects.

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::arch::asm;
use core::cmp::min;
use core::hint::spin_loop;
use core::ptr::{read_volatile, write_volatile};
use core::time::Duration;

use crate::common::ring_array::RingArray;
use crate::drivers::syscon::get_rpi_firmware;
use crate::drivers::{Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::node::Node;
use crate::kernel::irq::{InterruptHandler, enable_irq, register_handler, resolve_virq};
use crate::kernel::sync::{OnceLock, SpinLock, with_addr_lock, without_irq_fiq};
use crate::kernel::terminal::{InputDevice, register_input};
use crate::kprintln;
use crate::mm::map_io_region;

use super::core::{
    ControlTransferData, DeviceDescriptor, EndpointAddress, EndpointInfo, InterfaceDescriptor,
    InterruptInHandler, InterruptInTransfer, SetupPacket, USB_CLASS_HID, USB_CLASS_HUB,
    USB_DESC_CONFIGURATION, USB_DESC_DEVICE, USB_DESC_HUB, USB_HID_PROTOCOL_BOOT,
    USB_HID_PROTOCOL_KEYBOARD, USB_HID_SUBCLASS_BOOT, USB_HUB_PORT_POWER, USB_HUB_PORT_RESET,
    USB_REQ_GET_DESCRIPTOR, USB_REQ_RECIP_DEVICE, USB_REQ_RECIP_OTHER, USB_REQ_TYPE_CLASS,
    UsbDirection, UsbDmaBuffer, UsbEndpointType, UsbError, UsbHostController, UsbResult, UsbRoute,
    UsbSpeed, clear_feature, control_in, find_interface, find_interrupt_in_endpoint,
    get_descriptor, get_status, parse_configuration_descriptor, parse_device_descriptor,
    parse_device_descriptor_header, set_address, set_configuration, set_feature, set_idle,
    set_interface, set_protocol,
};
use super::register_host_controller;

const USB_POWER_STATE_ON: u32 = 1 << 0;
const USB_POWER_STATE_WAIT: u32 = 1 << 1;
const USB_HCD_DEVICE_ID: u32 = 0x0000_0003;

const CONTROL_CHANNEL: usize = 0;
const INTERRUPT_CHANNEL: usize = 1;
const KEYBOARD_REPORT_LEN: usize = 8;
const KEYBOARD_QUEUE_LEN: usize = 256;
const KEYBOARD_REPEAT_DELAY: Duration = Duration::from_millis(200);
const KEYBOARD_REPEAT_INTERVAL: Duration = Duration::from_millis(40);
/// Hard upper bound on how long a single held key may auto-repeat. A held key is only
/// released by one report edge; if that edge is ever lost (transfer torn down, dropped
/// report) the key would otherwise repeat forever. This fuse guarantees it cannot.
const KEYBOARD_REPEAT_MAX_HOLD: Duration = Duration::from_secs(10);

const SETUP_BUFFER_LEN: usize = 8;
const CONTROL_BUFFER_LEN: usize = 1024;

const USB_BUS_UNCACHED_ALIAS: u32 = 0xC000_0000;

const USB_CORE_AHB_CFG_OFFSET: usize = 0x008;
const USB_CORE_USB_CFG_OFFSET: usize = 0x00C;
const USB_CORE_RESET_OFFSET: usize = 0x010;
const USB_CORE_INT_STATUS_OFFSET: usize = 0x014;
const USB_CORE_INT_MASK_OFFSET: usize = 0x018;
const USB_CORE_HW_CFG2_OFFSET: usize = 0x048;

const USB_HOST_CFG_OFFSET: usize = 0x400;
const USB_HOST_FRAME_INTERVAL_OFFSET: usize = 0x404;
const USB_HOST_FRAME_NUMBER_OFFSET: usize = 0x408;
const USB_HOST_ALL_CHANNEL_INT_OFFSET: usize = 0x414;
const USB_HOST_ALL_CHANNEL_INT_MASK_OFFSET: usize = 0x418;
const USB_HOST_PORT_OFFSET: usize = 0x440;

const USB_POWER_OFFSET: usize = 0xE00;

const USB_HOST_CHANNEL_BASE_OFFSET: usize = 0x500;
const USB_HOST_CHANNEL_STRIDE: usize = 0x20;
const USB_HOST_CHANNEL_CHARACTER_OFFSET: usize = 0x00;
const USB_HOST_CHANNEL_SPLIT_CTRL_OFFSET: usize = 0x04;
const USB_HOST_CHANNEL_INT_OFFSET: usize = 0x08;
const USB_HOST_CHANNEL_INT_MASK_OFFSET: usize = 0x0C;
const USB_HOST_CHANNEL_XFER_SIZE_OFFSET: usize = 0x10;
const USB_HOST_CHANNEL_DMA_ADDR_OFFSET: usize = 0x14;

const GAHBCFG_GLBL_INTR_EN: u32 = 1 << 0;
const GAHBCFG_HBSTLEN_MASK: u32 = 0xF << 1;
const GAHBCFG_WAIT_AXI_WRITES: u32 = 1 << 4;
const GAHBCFG_DMA_EN: u32 = 1 << 5;

const GUSBCFG_PHYIF16: u32 = 1 << 3;
const GUSBCFG_ULPI_UTMI_SEL: u32 = 1 << 4;
const GUSBCFG_SRPCAP: u32 = 1 << 8;
const GUSBCFG_HNPCAP: u32 = 1 << 9;
const GUSBCFG_ULPI_FS_LS: u32 = 1 << 17;
const GUSBCFG_ULPI_CLK_SUSP_M: u32 = 1 << 19;
const GUSBCFG_ULPI_EXT_VBUS_DRV: u32 = 1 << 20;
const GUSBCFG_TERMSELDLPULSE: u32 = 1 << 22;
const GUSBCFG_FORCEHOSTMODE: u32 = 1 << 29;
const GUSBCFG_FORCEDEVMODE: u32 = 1 << 30;

const GRSTCTL_CSFTRST: u32 = 1 << 0;
const GRSTCTL_RXFFLSH: u32 = 1 << 4;
const GRSTCTL_TXFFLSH: u32 = 1 << 5;
const GRSTCTL_TXFNUM_SHIFT: u32 = 6;
const GRSTCTL_AHBIDLE: u32 = 1 << 31;

const GINTSTS_PRTINT: u32 = 1 << 24;
const GINTSTS_HCHINT: u32 = 1 << 25;
const GINTSTS_DISCONNINT: u32 = 1 << 29;

const GHWCFG2_ARCHITECTURE_SHIFT: u32 = 3;
const GHWCFG2_ARCHITECTURE_MASK: u32 = 0x3 << GHWCFG2_ARCHITECTURE_SHIFT;
const GHWCFG2_ARCHITECTURE_INT_DMA: u32 = 0x2;
const GHWCFG2_NUM_HOST_CHANNELS_SHIFT: u32 = 14;
const GHWCFG2_NUM_HOST_CHANNELS_MASK: u32 = 0xF << GHWCFG2_NUM_HOST_CHANNELS_SHIFT;

const HCFG_FSLS_PCLK_SEL_MASK: u32 = 0x3;
const HCFG_FSLS_PCLK_SEL_30_60_MHZ: u32 = 0x0;
const HOST_FRAME_INTERVAL_1MS: u32 = 48_000;

const HPRT_CONNECT: u32 = 1 << 0;
const HPRT_CONNECT_CHANGED: u32 = 1 << 1;
const HPRT_ENABLE: u32 = 1 << 2;
const HPRT_ENABLE_CHANGED: u32 = 1 << 3;
const HPRT_OVERCURRENT_CHANGED: u32 = 1 << 5;
const HPRT_RESET: u32 = 1 << 8;
const HPRT_POWER: u32 = 1 << 12;
const HPRT_SPEED_SHIFT: u32 = 17;
const HPRT_SPEED_MASK: u32 = 0x3 << HPRT_SPEED_SHIFT;
const HPRT_SPEED_HIGH: u32 = 0;
const HPRT_SPEED_FULL: u32 = 1;
const HPRT_SPEED_LOW: u32 = 2;
const HPRT_DEFAULT_MASK: u32 =
    HPRT_CONNECT_CHANGED | HPRT_ENABLE | HPRT_ENABLE_CHANGED | HPRT_OVERCURRENT_CHANGED;

const HCCHAR_MAX_PACKET_SIZE_MASK: u32 = 0x7FF;
const HCCHAR_ENDPOINT_NUMBER_SHIFT: u32 = 11;
const HCCHAR_ENDPOINT_DIRECTION_IN: u32 = 1 << 15;
const HCCHAR_LOW_SPEED_DEVICE: u32 = 1 << 17;
const HCCHAR_ENDPOINT_TYPE_SHIFT: u32 = 18;
const HCCHAR_MULTI_COUNT_SHIFT: u32 = 20;
const HCCHAR_DEVICE_ADDRESS_SHIFT: u32 = 22;
const HCCHAR_ODD_FRAME: u32 = 1 << 29;
const HCCHAR_DISABLE: u32 = 1 << 30;
const HCCHAR_ENABLE: u32 = 1 << 31;

const HCSPLT_PORT_ADDRESS_MASK: u32 = 0x7F;
const HCSPLT_HUB_ADDRESS_SHIFT: u32 = 7;
const HCSPLT_XACT_POSITION_SHIFT: u32 = 14;
const HCSPLT_XACT_POSITION_ALL: u32 = 0x3;
const HCSPLT_COMPLETE_SPLIT: u32 = 1 << 16;
const HCSPLT_SPLIT_ENABLE: u32 = 1 << 31;

const HCINT_XFER_COMPLETE: u32 = 1 << 0;
const HCINT_HALTED: u32 = 1 << 1;
const HCINT_AHB_ERROR: u32 = 1 << 2;
const HCINT_STALL: u32 = 1 << 3;
const HCINT_NAK: u32 = 1 << 4;
const HCINT_ACK: u32 = 1 << 5;
const HCINT_NYET: u32 = 1 << 6;
const HCINT_XACT_ERROR: u32 = 1 << 7;
const HCINT_BABBLE_ERROR: u32 = 1 << 8;
const HCINT_FRAME_OVERRUN: u32 = 1 << 9;
const HCINT_DATA_TOGGLE_ERROR: u32 = 1 << 10;
const HCINT_ERROR_MASK: u32 = HCINT_AHB_ERROR
    | HCINT_STALL
    | HCINT_XACT_ERROR
    | HCINT_BABBLE_ERROR
    | HCINT_FRAME_OVERRUN
    | HCINT_DATA_TOGGLE_ERROR;

const HCTSIZ_TRANSFER_SIZE_MASK: u32 = 0x7FFFF;
const HCTSIZ_PACKET_COUNT_SHIFT: u32 = 19;
const HCTSIZ_PID_SHIFT: u32 = 29;

const POLL_STEP: Duration = Duration::from_micros(50);
const RETRY_DELAY: Duration = Duration::from_micros(100);
const CORE_RESET_TIMEOUT: Duration = Duration::from_millis(100);
const FIFO_FLUSH_TIMEOUT: Duration = Duration::from_millis(10);
const PORT_CONNECT_SETTLE: Duration = Duration::from_millis(100);
const PORT_RESET_DURATION: Duration = Duration::from_millis(50);
const PORT_RESET_RECOVERY: Duration = Duration::from_millis(20);
const PORT_ENABLE_TIMEOUT: Duration = Duration::from_millis(100);
const CHANNEL_TIMEOUT: Duration = Duration::from_millis(100);
const FORCE_HOST_SETTLE: Duration = Duration::from_millis(50);
const MAX_TRANSACTION_RETRIES: usize = 256;
const MAX_SPLIT_COMPLETE_RETRIES: usize = 4;
/// Maximum time a single armed async interrupt-IN transaction may stay in flight without the
/// channel raising any interrupt. If exceeded, the channel is treated as wedged (e.g. a lost
/// host-channel interrupt) and force-recovered, so the keyboard cannot stop delivering reports
/// (which would also let a held key keep auto-repeating) until an unrelated event frees it.
const INTERRUPT_INFLIGHT_TIMEOUT: Duration = Duration::from_millis(50);
/// How many consecutive transient transaction errors an async interrupt transfer tolerates
/// before it is torn down. Transient split/transaction noise is common under load on real
/// hardware; tearing the transfer down on the first error would needlessly kill the keyboard.
const MAX_INTERRUPT_ERROR_RETRIES: usize = 8;

const HUB_PORT_STATUS_CONNECTION: u16 = 1 << 0;
const HUB_PORT_STATUS_ENABLED: u16 = 1 << 1;
const HUB_PORT_STATUS_POWERED: u16 = 1 << 8;
const HUB_PORT_STATUS_LOW_SPEED: u16 = 1 << 9;
const HUB_PORT_STATUS_HIGH_SPEED: u16 = 1 << 10;

const HUB_PORT_CHANGE_CONNECTION: u16 = 1 << 0;
const HUB_PORT_CHANGE_ENABLE: u16 = 1 << 1;
const HUB_PORT_CHANGE_RESET: u16 = 1 << 4;

const HUB_FEATURE_C_PORT_CONNECTION: u16 = 16;
const HUB_FEATURE_C_PORT_ENABLE: u16 = 17;
const HUB_FEATURE_C_PORT_RESET: u16 = 20;

const HID_MODIFIER_LEFT_SHIFT: u8 = 1 << 1;
const HID_MODIFIER_RIGHT_SHIFT: u8 = 1 << 5;
const HID_MODIFIER_RIGHT_ALT: u8 = 1 << 6;
const HID_USAGE_CAPS_LOCK: u8 = 0x39;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PacketId {
    Data0 = 0,
    Data1 = 2,
    Setup = 3,
}

impl PacketId {
    const fn bits(self) -> u32 {
        self as u32
    }

    fn toggled(self) -> Self {
        match self {
            Self::Data0 => Self::Data1,
            Self::Data1 => Self::Data0,
            Self::Setup => Self::Setup,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitPhase {
    None,
    Start,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InterruptPollPhase {
    Idle,
    InFlight,
    Recovering,
}

#[derive(Clone, Copy)]
struct ChannelRequest {
    route: UsbRoute,
    endpoint_number: u8,
    endpoint_type: UsbEndpointType,
    direction: UsbDirection,
    max_packet_size: u16,
    packet_id: PacketId,
    transfer_size: usize,
    odd_frame: bool,
    split_phase: SplitPhase,
}

struct ControlResources {
    setup_buffer: UsbDmaBuffer,
    data_buffer: UsbDmaBuffer,
}

impl ControlResources {
    fn new() -> UsbResult<Self> {
        Ok(Self {
            setup_buffer: UsbDmaBuffer::new_zeroed(SETUP_BUFFER_LEN)?,
            data_buffer: UsbDmaBuffer::new_zeroed(CONTROL_BUFFER_LEN)?,
        })
    }
}

struct AsyncInterruptState {
    transfer: InterruptInTransfer,
    packet_id: PacketId,
    split_phase: SplitPhase,
    poll_phase: InterruptPollPhase,
    last_request_len: usize,
    interval_ticks: u64,
    next_due: u64,
    split_complete_retries: usize,
    inflight_since: u64,
    error_retries: usize,
}

impl AsyncInterruptState {
    fn new(transfer: InterruptInTransfer, interval_ticks: u64, now: u64) -> Self {
        let split_phase = if transfer.route.uses_split_transactions() {
            SplitPhase::Start
        } else {
            SplitPhase::None
        };

        Self {
            transfer,
            packet_id: PacketId::Data0,
            split_phase,
            poll_phase: InterruptPollPhase::Idle,
            last_request_len: 0,
            interval_ticks,
            next_due: now,
            split_complete_retries: 0,
            inflight_since: now,
            error_retries: 0,
        }
    }

    fn reset_split_phase(&mut self) {
        self.split_phase = if self.transfer.route.uses_split_transactions() {
            SplitPhase::Start
        } else {
            SplitPhase::None
        };
        self.split_complete_retries = 0;
    }

    fn mark_idle_until_next_interval(&mut self, now: u64) {
        self.poll_phase = InterruptPollPhase::Idle;
        self.next_due = now.wrapping_add(self.interval_ticks);
        self.reset_split_phase();
    }

    fn is_due(&self, now: u64) -> bool {
        now.wrapping_sub(self.next_due) < (1u64 << 63)
    }
}

struct EnumeratedDevice {
    route: UsbRoute,
    max_packet_size0: u16,
    device_descriptor: DeviceDescriptor,
    configuration_bytes: Vec<u8>,
}

struct HubDescriptorInfo {
    num_ports: u8,
    power_on_to_power_good_ms: u64,
}

struct HubPortStatus {
    status: u16,
    change: u16,
}

struct KeyboardState {
    previous_report: [u8; KEYBOARD_REPORT_LEN],
    caps_lock: bool,
    queue: RingArray<u8, KEYBOARD_QUEUE_LEN>,
    repeat: Option<RepeatKey>,
    repeat_tracked: Option<u8>,
    repeat_anchor: Duration,
    repeat_last_emit: Duration,
}

#[derive(Clone, Copy)]
struct RepeatKey {
    usage: u8,
    byte: u8,
}

impl KeyboardState {
    fn handle_report(&mut self, report: [u8; KEYBOARD_REPORT_LEN]) {
        if Self::has_error_rollover(&report) {
            return;
        }

        let modifiers = report[0];
        for usage in report[2..].iter().copied().filter(|usage| *usage != 0) {
            if self.previous_report[2..].contains(&usage) {
                continue;
            }

            if usage == HID_USAGE_CAPS_LOCK {
                self.caps_lock = !self.caps_lock;
                continue;
            }

            if let Some(byte) = Self::translate_usage(usage, modifiers, self.caps_lock) {
                self.queue.push(byte);
                self.repeat = Some(RepeatKey { usage, byte });
            }
        }

        if let Some(repeat) = self.repeat {
            if !report[2..].contains(&repeat.usage) {
                self.repeat = None;
            } else if let Some(byte) = Self::translate_usage(repeat.usage, modifiers, self.caps_lock)
            {
                // The held key is still down but its modifiers (e.g. Shift) may have changed.
                // Keep auto-repeat emitting the currently-correct byte rather than the byte
                // captured when the key was first pressed.
                self.repeat = Some(RepeatKey {
                    usage: repeat.usage,
                    byte,
                });
            }
        }

        self.previous_report = report;
    }

    /// Emit auto-repeat bytes for a held key. Runs in task context (driven by `read`),
    /// because boot keyboards send no further reports while a key is simply held.
    fn service_repeat(&mut self, now: Duration) {
        let Some(repeat) = self.repeat else {
            self.repeat_tracked = None;
            return;
        };

        if self.repeat_tracked != Some(repeat.usage) {
            // New key being held; the initial byte was already emitted by `handle_report`.
            self.repeat_tracked = Some(repeat.usage);
            self.repeat_anchor = now;
            self.repeat_last_emit = now;
            return;
        }

        // Safety fuse: a key release is signalled by a single report edge. If that edge is
        // ever lost (e.g. the interrupt transfer is torn down on a channel error), no further
        // reports arrive and the key would repeat forever. Force-stop after a bounded hold.
        if now.saturating_sub(self.repeat_anchor) >= KEYBOARD_REPEAT_MAX_HOLD {
            self.stop_repeat();
            return;
        }

        if now.saturating_sub(self.repeat_anchor) >= KEYBOARD_REPEAT_DELAY
            && now.saturating_sub(self.repeat_last_emit) >= KEYBOARD_REPEAT_INTERVAL
        {
            self.queue.push(repeat.byte);
            self.repeat_last_emit = now;
        }
    }

    /// Stop any in-progress auto-repeat. Called when the input state becomes unreliable,
    /// e.g. when the keyboard's interrupt transfer is torn down by an error or disconnect.
    fn stop_repeat(&mut self) {
        self.repeat = None;
        self.repeat_tracked = None;
        self.previous_report = [0; KEYBOARD_REPORT_LEN];
    }

    fn has_error_rollover(report: &[u8; KEYBOARD_REPORT_LEN]) -> bool {        report[2..]
            .iter()
            .any(|usage| (0x01..=0x03).contains(usage))
    }

    fn translate_usage(usage: u8, modifiers: u8, caps_lock: bool) -> Option<u8> {
        let shift = (modifiers & (HID_MODIFIER_LEFT_SHIFT | HID_MODIFIER_RIGHT_SHIFT)) != 0;
        let alt_gr = (modifiers & HID_MODIFIER_RIGHT_ALT) != 0;

        if alt_gr {
            return match usage {
                0x14 => Some(b'@'),
                0x24 => Some(b'{'),
                0x25 => Some(b'['),
                0x26 => Some(b']'),
                0x27 => Some(b'}'),
                0x2d => Some(b'\\'),
                0x30 => Some(b'~'),
                0x64 => Some(b'|'),
                _ => None,
            };
        }

        match usage {
            0x04..=0x1d => Self::translate_letter(usage, shift ^ caps_lock),
            0x1e => Some(if shift { b'!' } else { b'1' }),
            0x1f => Some(if shift { b'"' } else { b'2' }),
            0x20 => {
                if shift {
                    None
                } else {
                    Some(b'3')
                }
            }
            0x21 => Some(if shift { b'$' } else { b'4' }),
            0x22 => Some(if shift { b'%' } else { b'5' }),
            0x23 => Some(if shift { b'&' } else { b'6' }),
            0x24 => Some(if shift { b'/' } else { b'7' }),
            0x25 => Some(if shift { b'(' } else { b'8' }),
            0x26 => Some(if shift { b')' } else { b'9' }),
            0x27 => Some(if shift { b'=' } else { b'0' }),
            0x28 => Some(b'\n'),
            0x29 => Some(0x1b),
            0x2a => Some(0x08),
            0x2b => Some(b'\t'),
            0x2c => Some(b' '),
            0x2d => {
                if shift {
                    Some(b'?')
                } else {
                    None
                }
            }
            0x30 => Some(if shift { b'*' } else { b'+' }),
            0x31 | 0x32 => Some(if shift { b'\'' } else { b'#' }),
            0x35 => {
                if shift {
                    None
                } else {
                    Some(b'^')
                }
            }
            0x36 => Some(if shift { b';' } else { b',' }),
            0x37 => Some(if shift { b':' } else { b'.' }),
            0x38 => Some(if shift { b'_' } else { b'-' }),
            0x64 => Some(if shift { b'>' } else { b'<' }),
            _ => None,
        }
    }

    fn translate_letter(usage: u8, uppercase: bool) -> Option<u8> {
        let mut letter = match usage {
            0x04..=0x1b => b'a' + (usage - 0x04),
            0x1c => b'z',
            0x1d => b'y',
            _ => return None,
        };

        if uppercase {
            letter = letter.to_ascii_uppercase();
        }

        Some(letter)
    }
}

struct UsbKeyboardDevice {
    id: String,
    controller: Arc<Bcm2708UsbDevice>,
    state: SpinLock<KeyboardState>,
}

impl UsbKeyboardDevice {
    fn new(id: String, controller: Arc<Bcm2708UsbDevice>) -> Self {
        Self {
            id,
            controller,
            state: SpinLock::new(KeyboardState {
                previous_report: [0; KEYBOARD_REPORT_LEN],
                caps_lock: false,
                queue: RingArray::new(0),
                repeat: None,
                repeat_tracked: None,
                repeat_anchor: Duration::ZERO,
                repeat_last_emit: Duration::ZERO,
            }),
        }
    }
}

impl Device for UsbKeyboardDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, _node: &Node) -> Result<(), DriverInitError> {
        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl InputDevice for UsbKeyboardDevice {
    fn read(&self) -> Vec<u8> {
        self.controller.service_interrupt_polls();

        let now = crate::kernel::time::uptime();
        let mut state = self.state.lock_irq();
        state.service_repeat(now);
        let len = state.queue.len();
        let mut output = vec![0u8; len];
        state.queue.drain(&mut output);
        output
    }
}

impl InterruptInHandler for UsbKeyboardDevice {
    fn handle_interrupt_in(&self, data: &[u8]) {
        if data.len() != KEYBOARD_REPORT_LEN {
            return;
        }

        let mut report = [0u8; KEYBOARD_REPORT_LEN];
        report.copy_from_slice(data);
        self.state.lock().handle_report(report);
    }

    fn handle_interrupt_error(&self, _error: UsbError) {
        // The interrupt transfer has been torn down, so no further reports (including the
        // pending key release) can arrive. Stop any auto-repeat so a key cannot stick.
        self.state.lock_irq().stop_repeat();
    }
}

pub struct Bcm2708UsbDevice {
    id: String,
    reg_base: usize,
    virq: u32,
    channel_count: OnceLock<usize>,
    control: SpinLock<ControlResources>,
    interrupt: SpinLock<Option<AsyncInterruptState>>,
}

impl Bcm2708UsbDevice {
    fn new(id: String, reg_base: usize, virq: u32) -> UsbResult<Self> {
        Ok(Self {
            id,
            reg_base,
            virq,
            channel_count: OnceLock::new(),
            control: SpinLock::new(ControlResources::new()?),
            interrupt: SpinLock::new(None),
        })
    }

    #[inline]
    fn channel_count(&self) -> usize {
        self.channel_count.get().copied().unwrap_or(0)
    }

    #[inline]
    fn channel_reg_offset(channel: usize, offset: usize) -> usize {
        USB_HOST_CHANNEL_BASE_OFFSET + channel * USB_HOST_CHANNEL_STRIDE + offset
    }

    fn with_reg_lock<F, R>(&self, offset: usize, f: F) -> R
    where
        F: FnOnce(*mut u32) -> R,
    {
        let reg = (self.reg_base + offset) as *mut u32;
        without_irq_fiq(|| with_addr_lock(reg as usize, || f(reg)))
    }

    #[inline]
    fn read_reg(&self, offset: usize) -> u32 {
        self.with_reg_lock(offset, |reg| {
            // SAFETY: `reg` is an MMIO register address derived from the controller's mapped base.
            unsafe { read_volatile(reg as *const u32) }
        })
    }

    #[inline]
    fn write_reg(&self, offset: usize, value: u32) {
        self.with_reg_lock(offset, |reg| {
            // SAFETY: `reg` is an MMIO register address derived from the controller's mapped base.
            unsafe { write_volatile(reg, value) };
        });
    }

    #[inline]
    fn read_channel_reg(&self, channel: usize, offset: usize) -> u32 {
        self.read_reg(Self::channel_reg_offset(channel, offset))
    }

    #[inline]
    fn write_channel_reg(&self, channel: usize, offset: usize, value: u32) {
        self.write_reg(Self::channel_reg_offset(channel, offset), value);
    }

    fn read_counter_frequency() -> u64 {
        let frequency: u64;
        // SAFETY: Reading `cntfrq_el0` is side-effect free and valid at EL1.
        unsafe {
            asm!("mrs {0}, cntfrq_el0", out(reg) frequency, options(nomem, nostack, preserves_flags));
        }
        frequency
    }

    fn read_counter() -> u64 {
        let counter: u64;
        // SAFETY: `isb` orders the architectural counter read that follows.
        unsafe {
            asm!("isb", options(nomem, nostack));
        }
        // SAFETY: Reading `cntvct_el0` is side-effect free and valid at EL1.
        unsafe {
            asm!("mrs {0}, cntvct_el0", out(reg) counter, options(nomem, nostack, preserves_flags));
        }
        counter
    }

    fn delay(duration: Duration) {
        if duration == Duration::ZERO {
            return;
        }

        let frequency = Self::read_counter_frequency();
        if frequency == 0 {
            for _ in 0..1_000 {
                spin_loop();
            }
            return;
        }

        let wait_ticks = ((duration.as_secs() as u128 * frequency as u128)
            + (duration.subsec_nanos() as u128 * frequency as u128 / 1_000_000_000u128))
            as u64;
        if wait_ticks == 0 {
            return;
        }

        let start = Self::read_counter();
        while Self::read_counter().wrapping_sub(start) < wait_ticks {
            spin_loop();
        }
    }

    fn duration_to_counter_ticks(duration: Duration) -> u64 {
        let frequency = Self::read_counter_frequency();
        if frequency == 0 {
            return 0;
        }

        ((duration.as_secs() as u128 * frequency as u128)
            + (duration.subsec_nanos() as u128 * frequency as u128 / 1_000_000_000u128))
            as u64
    }

    fn interrupt_interval_ticks(route: UsbRoute, endpoint: EndpointInfo) -> u64 {
        let interval = endpoint.interval.max(1);
        let duration = if route.speed() == UsbSpeed::High {
            let exponent = interval.min(16);
            let microframes = 1u64 << (exponent - 1);
            Duration::from_nanos(microframes.saturating_mul(125_000))
        } else {
            Duration::from_millis(u64::from(interval))
        };

        Self::duration_to_counter_ticks(duration).max(1)
    }

    fn wait_for_bits(
        &self,
        offset: usize,
        mask: u32,
        expected_set: bool,
        timeout: Duration,
    ) -> UsbResult<u32> {
        let start = Self::read_counter();
        let wait_ticks = ((timeout.as_secs() as u128 * Self::read_counter_frequency() as u128)
            + (timeout.subsec_nanos() as u128 * Self::read_counter_frequency() as u128
                / 1_000_000_000u128)) as u64;

        loop {
            let value = self.read_reg(offset);
            if ((value & mask) != 0) == expected_set {
                return Ok(value);
            }

            if wait_ticks != 0 && Self::read_counter().wrapping_sub(start) >= wait_ticks {
                return Err(UsbError::Timeout);
            }

            Self::delay(POLL_STEP);
        }
    }

    fn power_on_hcd(&self) -> UsbResult<()> {
        let firmware = get_rpi_firmware().ok_or(UsbError::InvalidState)?;
        let state = firmware
            .set_power_state(USB_HCD_DEVICE_ID, USB_POWER_STATE_ON | USB_POWER_STATE_WAIT)
            .map_err(|_| UsbError::HardwareFault)?;

        if (state & USB_POWER_STATE_ON) == 0 {
            return Err(UsbError::HardwareFault);
        }

        Ok(())
    }

    fn soft_reset(&self) -> UsbResult<()> {
        self.wait_for_bits(
            USB_CORE_RESET_OFFSET,
            GRSTCTL_AHBIDLE,
            true,
            CORE_RESET_TIMEOUT,
        )?;

        let mut reset = self.read_reg(USB_CORE_RESET_OFFSET);
        reset |= GRSTCTL_CSFTRST;
        self.write_reg(USB_CORE_RESET_OFFSET, reset);

        self.wait_for_bits(
            USB_CORE_RESET_OFFSET,
            GRSTCTL_CSFTRST,
            false,
            FIFO_FLUSH_TIMEOUT,
        )?;
        Self::delay(CORE_RESET_TIMEOUT);
        Ok(())
    }

    fn flush_tx_fifo(&self, fifo: u32) -> UsbResult<()> {
        let mut reset = self.read_reg(USB_CORE_RESET_OFFSET);
        reset |= GRSTCTL_TXFFLSH;
        reset &= !(0x1F << GRSTCTL_TXFNUM_SHIFT);
        reset |= fifo << GRSTCTL_TXFNUM_SHIFT;
        self.write_reg(USB_CORE_RESET_OFFSET, reset);

        self.wait_for_bits(
            USB_CORE_RESET_OFFSET,
            GRSTCTL_TXFFLSH,
            false,
            FIFO_FLUSH_TIMEOUT,
        )?;
        Self::delay(Duration::from_micros(1));
        Ok(())
    }

    fn flush_rx_fifo(&self) -> UsbResult<()> {
        let mut reset = self.read_reg(USB_CORE_RESET_OFFSET);
        reset |= GRSTCTL_RXFFLSH;
        self.write_reg(USB_CORE_RESET_OFFSET, reset);

        self.wait_for_bits(
            USB_CORE_RESET_OFFSET,
            GRSTCTL_RXFFLSH,
            false,
            FIFO_FLUSH_TIMEOUT,
        )?;
        Self::delay(Duration::from_micros(1));
        Ok(())
    }

    fn init_core(&self) -> UsbResult<usize> {
        let mut usb_cfg = self.read_reg(USB_CORE_USB_CFG_OFFSET);
        usb_cfg &= !(GUSBCFG_ULPI_EXT_VBUS_DRV | GUSBCFG_TERMSELDLPULSE | GUSBCFG_FORCEDEVMODE);
        usb_cfg |= GUSBCFG_FORCEHOSTMODE;
        self.write_reg(USB_CORE_USB_CFG_OFFSET, usb_cfg);
        Self::delay(FORCE_HOST_SETTLE);

        self.soft_reset()?;

        usb_cfg = self.read_reg(USB_CORE_USB_CFG_OFFSET);
        usb_cfg &= !(GUSBCFG_ULPI_UTMI_SEL
            | GUSBCFG_PHYIF16
            | GUSBCFG_ULPI_FS_LS
            | GUSBCFG_ULPI_CLK_SUSP_M
            | GUSBCFG_HNPCAP
            | GUSBCFG_SRPCAP
            | GUSBCFG_FORCEDEVMODE);
        usb_cfg |= GUSBCFG_FORCEHOSTMODE;
        self.write_reg(USB_CORE_USB_CFG_OFFSET, usb_cfg);

        let hw_cfg2 = self.read_reg(USB_CORE_HW_CFG2_OFFSET);
        let architecture = (hw_cfg2 & GHWCFG2_ARCHITECTURE_MASK) >> GHWCFG2_ARCHITECTURE_SHIFT;
        if architecture != GHWCFG2_ARCHITECTURE_INT_DMA {
            return Err(UsbError::Unsupported);
        }

        let channel_count = ((hw_cfg2 & GHWCFG2_NUM_HOST_CHANNELS_MASK)
            >> GHWCFG2_NUM_HOST_CHANNELS_SHIFT) as usize
            + 1;
        if channel_count < 2 {
            return Err(UsbError::Unsupported);
        }

        let mut ahb_cfg = self.read_reg(USB_CORE_AHB_CFG_OFFSET);
        ahb_cfg &= !GAHBCFG_HBSTLEN_MASK;
        ahb_cfg |= GAHBCFG_WAIT_AXI_WRITES | GAHBCFG_DMA_EN;
        self.write_reg(USB_CORE_AHB_CFG_OFFSET, ahb_cfg);

        self.write_reg(USB_CORE_INT_STATUS_OFFSET, u32::MAX);
        Ok(channel_count)
    }

    fn init_host(&self) -> UsbResult<()> {
        self.write_reg(USB_POWER_OFFSET, 0);

        let mut host_cfg = self.read_reg(USB_HOST_CFG_OFFSET);
        host_cfg &= !HCFG_FSLS_PCLK_SEL_MASK;
        host_cfg |= HCFG_FSLS_PCLK_SEL_30_60_MHZ;
        self.write_reg(USB_HOST_CFG_OFFSET, host_cfg);
        self.write_reg(USB_HOST_FRAME_INTERVAL_OFFSET, HOST_FRAME_INTERVAL_1MS);

        self.flush_tx_fifo(0x10)?;
        self.flush_rx_fifo()?;

        self.update_host_port(HPRT_POWER, 0, 0);
        self.acknowledge_host_port_changes();

        for channel in 0..self.channel_count() {
            self.write_channel_reg(channel, USB_HOST_CHANNEL_INT_MASK_OFFSET, 0);
            self.write_channel_reg(channel, USB_HOST_CHANNEL_INT_OFFSET, u32::MAX);
        }

        self.write_reg(USB_HOST_ALL_CHANNEL_INT_MASK_OFFSET, 0);
        self.write_reg(
            USB_CORE_INT_MASK_OFFSET,
            GINTSTS_HCHINT | GINTSTS_PRTINT | GINTSTS_DISCONNINT,
        );

        let mut ahb_cfg = self.read_reg(USB_CORE_AHB_CFG_OFFSET);
        ahb_cfg |= GAHBCFG_GLBL_INTR_EN;
        self.write_reg(USB_CORE_AHB_CFG_OFFSET, ahb_cfg);

        Ok(())
    }

    fn update_host_port(&self, set_bits: u32, clear_bits: u32, ack_bits: u32) {
        let mut port = self.read_reg(USB_HOST_PORT_OFFSET);
        port &= !HPRT_DEFAULT_MASK;
        port |= set_bits | ack_bits;
        port &= !clear_bits;
        self.write_reg(USB_HOST_PORT_OFFSET, port);
    }

    fn acknowledge_host_port_changes(&self) {
        let port = self.read_reg(USB_HOST_PORT_OFFSET);
        let ack = port & (HPRT_CONNECT_CHANGED | HPRT_ENABLE_CHANGED | HPRT_OVERCURRENT_CHANGED);
        if ack != 0 {
            self.update_host_port(0, 0, ack);
        }
    }

    fn map_port_speed(&self) -> UsbResult<UsbSpeed> {
        let port = self.read_reg(USB_HOST_PORT_OFFSET);
        match (port & HPRT_SPEED_MASK) >> HPRT_SPEED_SHIFT {
            HPRT_SPEED_HIGH => Ok(UsbSpeed::High),
            HPRT_SPEED_FULL => Ok(UsbSpeed::Full),
            HPRT_SPEED_LOW => Ok(UsbSpeed::Low),
            _ => Err(UsbError::Unsupported),
        }
    }

    fn bus_address(phys_addr: usize) -> UsbResult<u32> {
        let phys_addr = u32::try_from(phys_addr).map_err(|_| UsbError::Unsupported)?;
        Ok((phys_addr & !USB_BUS_UNCACHED_ALIAS) | USB_BUS_UNCACHED_ALIAS)
    }

    fn endpoint_type_bits(endpoint_type: UsbEndpointType) -> u32 {
        match endpoint_type {
            UsbEndpointType::Control => 0,
            UsbEndpointType::Isochronous => 1,
            UsbEndpointType::Bulk => 2,
            UsbEndpointType::Interrupt => 3,
        }
    }

    fn programmed_transfer_size(request: ChannelRequest) -> usize {
        if request.route.uses_split_transactions() {
            if request.split_phase == SplitPhase::Complete
                && request.direction != UsbDirection::In
            {
                return 0;
            }

            if request.direction == UsbDirection::In {
                return request.transfer_size.max(usize::from(request.max_packet_size));
            }
        }

        request.transfer_size
    }

    fn next_odd_frame(&self) -> bool {
        (self.read_reg(USB_HOST_FRAME_NUMBER_OFFSET) & 1) == 0
    }

    fn configure_channel_irq(&self, channel: usize, mask: u32) {
        self.write_channel_reg(channel, USB_HOST_CHANNEL_INT_MASK_OFFSET, mask);

        let mut all_mask = self.read_reg(USB_HOST_ALL_CHANNEL_INT_MASK_OFFSET);
        if mask == 0 {
            all_mask &= !(1 << channel);
        } else {
            all_mask |= 1 << channel;
        }
        self.write_reg(USB_HOST_ALL_CHANNEL_INT_MASK_OFFSET, all_mask);
    }

    fn halt_channel(&self, channel: usize) -> UsbResult<()> {
        let hcchar = self.read_channel_reg(channel, USB_HOST_CHANNEL_CHARACTER_OFFSET);
        if (hcchar & HCCHAR_ENABLE) == 0 {
            return Ok(());
        }

        self.write_channel_reg(
            channel,
            USB_HOST_CHANNEL_CHARACTER_OFFSET,
            hcchar | HCCHAR_DISABLE | HCCHAR_ENABLE,
        );

        let start = Self::read_counter();
        let wait_ticks = ((CHANNEL_TIMEOUT.as_secs() as u128
            * Self::read_counter_frequency() as u128)
            + (CHANNEL_TIMEOUT.subsec_nanos() as u128 * Self::read_counter_frequency() as u128
                / 1_000_000_000u128)) as u64;

        loop {
            let hcint = self.read_channel_reg(channel, USB_HOST_CHANNEL_INT_OFFSET);
            if (hcint & HCINT_HALTED) != 0 {
                self.write_channel_reg(channel, USB_HOST_CHANNEL_INT_OFFSET, hcint);
                return Ok(());
            }

            if wait_ticks != 0 && Self::read_counter().wrapping_sub(start) >= wait_ticks {
                let hctsiz = self.read_channel_reg(channel, USB_HOST_CHANNEL_XFER_SIZE_OFFSET);
                let hprt = self.read_reg(USB_HOST_PORT_OFFSET);
                kprintln!(
                    "[WARNING][{}] USB channel {} timeout: hcint={:#010x}, hcchar={:#010x}, hctsiz={:#010x}, hprt={:#010x}",
                    self.id(),
                    channel,
                    hcint,
                    hcchar,
                    hctsiz,
                    hprt
                );
                return Err(UsbError::Timeout);
            }

            Self::delay(POLL_STEP);
        }
    }

    fn arm_channel(
        &self,
        channel: usize,
        request: ChannelRequest,
        dma_addr: u32,
        irq_mask: u32,
    ) -> UsbResult<()> {
        if channel >= self.channel_count() {
            return Err(UsbError::Unsupported);
        }

        self.halt_channel(channel).ok();
        self.write_channel_reg(channel, USB_HOST_CHANNEL_INT_OFFSET, u32::MAX);
        self.configure_channel_irq(channel, irq_mask);

        let split_ctrl = if request.route.uses_split_transactions() {
            let mut split = u32::from(request.route.parent_port()) & HCSPLT_PORT_ADDRESS_MASK;
            split |= u32::from(request.route.parent_hub_address()) << HCSPLT_HUB_ADDRESS_SHIFT;
            split |= HCSPLT_XACT_POSITION_ALL << HCSPLT_XACT_POSITION_SHIFT;
            if request.split_phase == SplitPhase::Complete {
                split |= HCSPLT_COMPLETE_SPLIT;
            }
            split | HCSPLT_SPLIT_ENABLE
        } else {
            0
        };
        self.write_channel_reg(channel, USB_HOST_CHANNEL_SPLIT_CTRL_OFFSET, split_ctrl);

        let packet_count = 1u32;
        let programmed_transfer_size = Self::programmed_transfer_size(request);
        let transfer_size =
            u32::try_from(programmed_transfer_size).map_err(|_| UsbError::BufferTooLarge)?;
        let xfer_size = (transfer_size & HCTSIZ_TRANSFER_SIZE_MASK)
            | (packet_count << HCTSIZ_PACKET_COUNT_SHIFT)
            | (request.packet_id.bits() << HCTSIZ_PID_SHIFT);
        self.write_channel_reg(channel, USB_HOST_CHANNEL_XFER_SIZE_OFFSET, xfer_size);
        self.write_channel_reg(channel, USB_HOST_CHANNEL_DMA_ADDR_OFFSET, dma_addr);

        let mut hcchar = u32::from(request.max_packet_size) & HCCHAR_MAX_PACKET_SIZE_MASK;
        hcchar |= u32::from(request.endpoint_number) << HCCHAR_ENDPOINT_NUMBER_SHIFT;
        hcchar |= Self::endpoint_type_bits(request.endpoint_type) << HCCHAR_ENDPOINT_TYPE_SHIFT;
        hcchar |= 1 << HCCHAR_MULTI_COUNT_SHIFT;
        hcchar |= u32::from(request.route.address()) << HCCHAR_DEVICE_ADDRESS_SHIFT;
        if request.direction == UsbDirection::In {
            hcchar |= HCCHAR_ENDPOINT_DIRECTION_IN;
        }
        if request.route.speed() == UsbSpeed::Low {
            hcchar |= HCCHAR_LOW_SPEED_DEVICE;
        }
        if request.endpoint_type == UsbEndpointType::Interrupt && request.odd_frame {
            hcchar |= HCCHAR_ODD_FRAME;
        }
        hcchar |= HCCHAR_ENABLE;
        self.write_channel_reg(channel, USB_HOST_CHANNEL_CHARACTER_OFFSET, hcchar);

        Ok(())
    }

    fn wait_for_channel_result(&self, channel: usize, timeout: Duration) -> UsbResult<u32> {
        self.wait_for_channel_result_inner(channel, timeout, true, false)
    }

    fn wait_for_channel_result_quiet(&self, channel: usize, timeout: Duration) -> UsbResult<u32> {
        self.wait_for_channel_result_inner(channel, timeout, false, false)
    }

    fn wait_for_split_start_result(&self, channel: usize, timeout: Duration) -> UsbResult<u32> {
        self.wait_for_channel_result_inner(channel, timeout, true, true)
    }

    fn wait_for_split_start_result_quiet(
        &self,
        channel: usize,
        timeout: Duration,
    ) -> UsbResult<u32> {
        self.wait_for_channel_result_inner(channel, timeout, false, true)
    }

    fn wait_for_channel_result_inner(
        &self,
        channel: usize,
        timeout: Duration,
        log_timeout: bool,
        halt_on_ack: bool,
    ) -> UsbResult<u32> {
        let start = Self::read_counter();
        let wait_ticks = ((timeout.as_secs() as u128 * Self::read_counter_frequency() as u128)
            + (timeout.subsec_nanos() as u128 * Self::read_counter_frequency() as u128
                / 1_000_000_000u128)) as u64;
        let mut ack_halt_requested = false;

        loop {
            let hcint = self.read_channel_reg(channel, USB_HOST_CHANNEL_INT_OFFSET);
            let hcchar = self.read_channel_reg(channel, USB_HOST_CHANNEL_CHARACTER_OFFSET);
            let interrupt_found = (hcint
                & (HCINT_HALTED
                    | HCINT_XFER_COMPLETE
                    | HCINT_ERROR_MASK
                    | HCINT_NAK
                    | HCINT_ACK
                    | HCINT_NYET))
                != 0;
            if interrupt_found {
                if (hcint & HCINT_HALTED) != 0 || (hcchar & HCCHAR_ENABLE) == 0 {
                    return Ok(hcint);
                }

                if halt_on_ack && !ack_halt_requested && (hcint & HCINT_ACK) != 0 {
                    self.write_channel_reg(
                        channel,
                        USB_HOST_CHANNEL_CHARACTER_OFFSET,
                        hcchar | HCCHAR_DISABLE | HCCHAR_ENABLE,
                    );
                    ack_halt_requested = true;
                }
            }

            if wait_ticks != 0 && Self::read_counter().wrapping_sub(start) >= wait_ticks {
                if log_timeout {
                    let hctsiz = self.read_channel_reg(channel, USB_HOST_CHANNEL_XFER_SIZE_OFFSET);
                    let hprt = self.read_reg(USB_HOST_PORT_OFFSET);
                    kprintln!(
                        "[WARNING][{}] USB channel {} timeout: hcint={:#010x}, hcchar={:#010x}, hctsiz={:#010x}, hprt={:#010x}",
                        self.id(),
                        channel,
                        hcint,
                        hcchar,
                        hctsiz,
                        hprt
                    );
                }
                return Err(UsbError::Timeout);
            }

            Self::delay(POLL_STEP);
        }
    }

    fn translate_channel_error(&self, hcint: u32) -> UsbError {
        if (hcint & HCINT_STALL) != 0 {
            UsbError::Stall
        } else if !self.root_port_connected() {
            UsbError::Disconnected
        } else {
            UsbError::HardwareFault
        }
    }

    fn run_single_transaction_polling(
        &self,
        channel: usize,
        request: ChannelRequest,
        dma_addr: u32,
    ) -> UsbResult<usize> {
        let mut last_hcint = 0;
        let mut last_hcchar = 0;
        let mut last_hctsiz = 0;
        let mut last_hcsplt = 0;

        for _ in 0..MAX_TRANSACTION_RETRIES {
            self.arm_channel(channel, request, dma_addr, 0)?;

            let hcint = if request.route.uses_split_transactions()
                && request.split_phase == SplitPhase::Start
            {
                self.wait_for_split_start_result(channel, CHANNEL_TIMEOUT)?
            } else {
                self.wait_for_channel_result(channel, CHANNEL_TIMEOUT)?
            };
            let hctsiz = self.read_channel_reg(channel, USB_HOST_CHANNEL_XFER_SIZE_OFFSET);
            let hcchar = self.read_channel_reg(channel, USB_HOST_CHANNEL_CHARACTER_OFFSET);
            let hcsplt = self.read_channel_reg(channel, USB_HOST_CHANNEL_SPLIT_CTRL_OFFSET);
            last_hcint = hcint;
            last_hcchar = hcchar;
            last_hctsiz = hctsiz;
            last_hcsplt = hcsplt;
            self.write_channel_reg(channel, USB_HOST_CHANNEL_INT_OFFSET, hcint);

            if (hcint & HCINT_ERROR_MASK) != 0 {
                return Err(self.translate_channel_error(hcint));
            }

            if request.route.uses_split_transactions()
                && request.split_phase == SplitPhase::Complete
                && (hcint & HCINT_NYET) != 0
            {
                Self::delay(RETRY_DELAY);
                continue;
            }

            if request.route.uses_split_transactions()
                && request.split_phase == SplitPhase::Complete
                && (hcint & HCINT_NAK) != 0
            {
                return Err(UsbError::Busy);
            }

            if (hcint & (HCINT_NAK | HCINT_NYET)) != 0 {
                Self::delay(RETRY_DELAY);
                continue;
            }

            if request.split_phase == SplitPhase::Start {
                return Ok(0);
            }

            if request.split_phase == SplitPhase::Complete
                && request.direction != UsbDirection::In
                && (hcint & (HCINT_ACK | HCINT_XFER_COMPLETE)) != 0
            {
                return Ok(request.transfer_size);
            }

            let programmed_transfer_size = Self::programmed_transfer_size(request);
            let remaining = (hctsiz & HCTSIZ_TRANSFER_SIZE_MASK) as usize;
            let actual =
                min(programmed_transfer_size.saturating_sub(remaining), request.transfer_size);
            if (hcint & HCINT_XFER_COMPLETE) != 0 || request.transfer_size == 0 || actual > 0 {
                return Ok(actual);
            }

            Self::delay(RETRY_DELAY);
        }

        let hcint = self.read_channel_reg(channel, USB_HOST_CHANNEL_INT_OFFSET);
        let hcchar = self.read_channel_reg(channel, USB_HOST_CHANNEL_CHARACTER_OFFSET);
        let hctsiz = self.read_channel_reg(channel, USB_HOST_CHANNEL_XFER_SIZE_OFFSET);
        let hcsplt = self.read_channel_reg(channel, USB_HOST_CHANNEL_SPLIT_CTRL_OFFSET);
        kprintln!(
            "[WARNING][{}] USB transaction retries exhausted: ch={}, addr={}, ep={}, type={:?}, dir={:?}, speed={:?}, split={:?}, last_hcint={:#010x}, last_hcchar={:#010x}, last_hctsiz={:#010x}, last_hcsplt={:#010x}, hcint={:#010x}, hcchar={:#010x}, hctsiz={:#010x}, hcsplt={:#010x}",
            self.id(),
            channel,
            request.route.address(),
            request.endpoint_number,
            request.endpoint_type,
            request.direction,
            request.route.speed(),
            request.split_phase,
            last_hcint,
            last_hcchar,
            last_hctsiz,
            last_hcsplt,
            hcint,
            hcchar,
            hctsiz,
            hcsplt
        );
        Err(UsbError::Timeout)
    }

    fn transfer_packet_polling(
        &self,
        channel: usize,
        route: UsbRoute,
        endpoint_number: u8,
        endpoint_type: UsbEndpointType,
        direction: UsbDirection,
        max_packet_size: u16,
        packet_id: PacketId,
        transfer_size: usize,
        dma_addr: u32,
    ) -> UsbResult<usize> {
        let request = ChannelRequest {
            route,
            endpoint_number,
            endpoint_type,
            direction,
            max_packet_size,
            packet_id,
            transfer_size,
            odd_frame: endpoint_type == UsbEndpointType::Interrupt && self.next_odd_frame(),
            split_phase: SplitPhase::None,
        };

        if !route.uses_split_transactions() {
            return self.run_single_transaction_polling(channel, request, dma_addr);
        }

        let mut start_request = request;
        start_request.split_phase = SplitPhase::Start;
        let mut complete_request = request;
        complete_request.odd_frame =
            endpoint_type == UsbEndpointType::Interrupt && self.next_odd_frame();
        complete_request.split_phase = SplitPhase::Complete;

        for _ in 0..MAX_TRANSACTION_RETRIES {
            self.run_single_transaction_polling(channel, start_request, dma_addr)?;

            match self.run_single_transaction_polling(channel, complete_request, dma_addr) {
                Ok(actual) => return Ok(actual),
                Err(UsbError::Busy) => {
                    Self::delay(RETRY_DELAY);
                    continue;
                }
                Err(error) => return Err(error),
            }
        }

        Err(UsbError::Timeout)
    }

    fn start_interrupt_locked(&self, state: &mut AsyncInterruptState) -> UsbResult<()> {
        let request_len = min(
            state.transfer.expected_len,
            usize::from(state.transfer.endpoint.max_packet_size),
        );
        state.last_request_len = request_len;

        state.transfer.buffer.clear();
        state.transfer.buffer.clean_cache();

        let request = ChannelRequest {
            route: state.transfer.route,
            endpoint_number: state.transfer.endpoint.address.number(),
            endpoint_type: UsbEndpointType::Interrupt,
            direction: UsbDirection::In,
            max_packet_size: state.transfer.endpoint.max_packet_size,
            packet_id: state.packet_id,
            transfer_size: request_len,
            odd_frame: self.next_odd_frame(),
            split_phase: state.split_phase,
        };

        let irq_mask = HCINT_HALTED
            | HCINT_XFER_COMPLETE
            | HCINT_STALL
            | HCINT_NAK
            | HCINT_ACK
            | HCINT_NYET
            | HCINT_XACT_ERROR
            | HCINT_AHB_ERROR
            | HCINT_BABBLE_ERROR
            | HCINT_FRAME_OVERRUN
            | HCINT_DATA_TOGGLE_ERROR;
        let dma_addr = Self::bus_address(state.transfer.buffer.phys_addr())?;
        self.arm_channel(INTERRUPT_CHANNEL, request, dma_addr, irq_mask)?;
        state.poll_phase = InterruptPollPhase::InFlight;
        state.inflight_since = Self::read_counter();
        Ok(())
    }

    fn service_interrupt_polls(&self) {
        let mut needs_recovery = false;
        {
            let mut interrupt = self.interrupt.lock_irq();
            let Some(state) = interrupt.as_mut() else {
                return;
            };

            match state.poll_phase {
                // A concurrent poller is already recovering the channel; leave it alone.
                InterruptPollPhase::Recovering => return,
                InterruptPollPhase::InFlight => {
                    let now = Self::read_counter();
                    let timeout = Self::duration_to_counter_ticks(INTERRUPT_INFLIGHT_TIMEOUT);
                    if timeout == 0 || now.wrapping_sub(state.inflight_since) < timeout {
                        return;
                    }

                    // Watchdog: the armed transaction produced no interrupt within the timeout,
                    // so the host channel is wedged (e.g. a lost host-channel interrupt). Mask
                    // the channel and mark the transfer as recovering so no other poller re-arms
                    // it, then halt it below outside this IRQ-disabled section. Halting here
                    // would block interrupts for up to 100ms.
                    self.configure_channel_irq(INTERRUPT_CHANNEL, 0);
                    state.reset_split_phase();
                    state.poll_phase = InterruptPollPhase::Recovering;
                    needs_recovery = true;
                }
                InterruptPollPhase::Idle => {
                    let now = Self::read_counter();
                    if !state.is_due(now) {
                        return;
                    }

                    if self.start_interrupt_locked(state).is_err() {
                        drop(interrupt);
                        self.stop_interrupt_with_error(UsbError::HardwareFault);
                    }
                    return;
                }
            }
        }

        if needs_recovery {
            self.recover_wedged_interrupt_channel();
        }
    }

    /// Halt a wedged async interrupt channel after the in-flight watchdog fired. Runs with
    /// interrupts enabled (the caller dropped the `interrupt` lock first) so the up-to-100ms
    /// channel halt cannot freeze the rest of the system. The phase is `Recovering` and the
    /// channel IRQ is masked, so neither another poller nor `handle_interrupt_channel_irq` can
    /// race this. Afterwards the transfer returns to `Idle` and is due, so the next poll re-arms.
    fn recover_wedged_interrupt_channel(&self) {
        let halt = self.halt_channel(INTERRUPT_CHANNEL);
        let hcint = self.read_channel_reg(INTERRUPT_CHANNEL, USB_HOST_CHANNEL_INT_OFFSET);
        self.write_channel_reg(INTERRUPT_CHANNEL, USB_HOST_CHANNEL_INT_OFFSET, u32::MAX);
        kprintln!(
            "[WARNING][{}] USB interrupt channel watchdog fired; re-arming (halt={:?}, hcint={:#010x})",
            self.id(),
            halt,
            hcint
        );

        let mut interrupt = self.interrupt.lock_irq();
        if let Some(state) = interrupt.as_mut() {
            state.poll_phase = InterruptPollPhase::Idle;
            state.next_due = Self::read_counter();
        }
    }

    fn stop_interrupt_with_error(&self, error: UsbError) {
        self.halt_channel(INTERRUPT_CHANNEL).ok();
        self.configure_channel_irq(INTERRUPT_CHANNEL, 0);

        let state = {
            let mut interrupt = self.interrupt.lock_irq();
            interrupt.take()
        };

        if let Some(state) = state {
            state.transfer.handler.handle_interrupt_error(error);
        }
    }

    fn handle_port_interrupt(&self) {
        let port = self.read_reg(USB_HOST_PORT_OFFSET);
        self.acknowledge_host_port_changes();

        if (port & HPRT_CONNECT) == 0 {
            self.stop_interrupt_with_error(UsbError::Disconnected);
        }
    }

    fn handle_interrupt_channel_irq(&self) {
        let hcint = self.read_channel_reg(INTERRUPT_CHANNEL, USB_HOST_CHANNEL_INT_OFFSET);
        if hcint == 0 {
            return;
        }

        let mut interrupt = self.interrupt.lock();
        let Some(state) = interrupt.as_mut() else {
            self.write_channel_reg(INTERRUPT_CHANNEL, USB_HOST_CHANNEL_INT_OFFSET, hcint);
            self.configure_channel_irq(INTERRUPT_CHANNEL, 0);
            return;
        };

        // The watchdog owns the channel during recovery: it halts the channel with interrupts
        // enabled, so a stray channel IRQ could land here. Leave HCINT and state untouched so
        // recovery can observe the HALTED bit and re-arm cleanly.
        if state.poll_phase == InterruptPollPhase::Recovering {
            return;
        }

        self.write_channel_reg(INTERRUPT_CHANNEL, USB_HOST_CHANNEL_INT_OFFSET, hcint);

        if (hcint & HCINT_ERROR_MASK) != 0 {
            let error = self.translate_channel_error(hcint);
            // Transient transaction noise (bad CRC/timeout, frame overrun, toggle mismatch) is
            // common under load on real split-transaction hardware. Retry such errors at the
            // next interval instead of tearing the transfer down; only persistent errors or
            // hard faults (stall, AHB, babble, disconnect) are fatal. Without this, a single
            // transient error would permanently kill the keyboard.
            let transient = (hcint
                & (HCINT_XACT_ERROR | HCINT_FRAME_OVERRUN | HCINT_DATA_TOGGLE_ERROR))
                != 0
                && (hcint & (HCINT_STALL | HCINT_AHB_ERROR | HCINT_BABBLE_ERROR)) == 0
                && self.root_port_connected();

            if transient && state.error_retries < MAX_INTERRUPT_ERROR_RETRIES {
                state.error_retries += 1;
                state.mark_idle_until_next_interval(Self::read_counter());
                self.configure_channel_irq(INTERRUPT_CHANNEL, 0);
                return;
            }

            drop(interrupt);
            self.stop_interrupt_with_error(error);
            return;
        }

        state.error_retries = 0;

        if (hcint & (HCINT_NAK | HCINT_NYET)) != 0 {
            if state.transfer.route.uses_split_transactions()
                && state.split_phase == SplitPhase::Complete
                && (hcint & HCINT_NYET) != 0
            {
                if state.split_complete_retries < MAX_SPLIT_COMPLETE_RETRIES {
                    state.split_complete_retries += 1;
                    if self.start_interrupt_locked(state).is_err() {
                        drop(interrupt);
                        self.stop_interrupt_with_error(UsbError::HardwareFault);
                    }
                } else {
                    state.mark_idle_until_next_interval(Self::read_counter());
                    self.configure_channel_irq(INTERRUPT_CHANNEL, 0);
                }
            } else {
                state.mark_idle_until_next_interval(Self::read_counter());
                self.configure_channel_irq(INTERRUPT_CHANNEL, 0);
            }
            return;
        }

        if state.transfer.route.uses_split_transactions() && state.split_phase == SplitPhase::Start
        {
            state.split_phase = SplitPhase::Complete;
            state.split_complete_retries = 0;
            if self.start_interrupt_locked(state).is_err() {
                drop(interrupt);
                self.stop_interrupt_with_error(UsbError::HardwareFault);
            }
            return;
        }

        let hctsiz = self.read_channel_reg(INTERRUPT_CHANNEL, USB_HOST_CHANNEL_XFER_SIZE_OFFSET);
        let remaining = (hctsiz & HCTSIZ_TRANSFER_SIZE_MASK) as usize;
        let actual_len = min(
            state.last_request_len.saturating_sub(remaining),
            state.transfer.expected_len,
        );

        state.transfer.buffer.invalidate_cache();
        state.packet_id = state.packet_id.toggled();
        if actual_len > 0 {
            state
                .transfer
                .handler
                .handle_interrupt_in(&state.transfer.buffer.as_slice()[..actual_len]);
        }

        state.mark_idle_until_next_interval(Self::read_counter());
        self.configure_channel_irq(INTERRUPT_CHANNEL, 0);
    }

    fn route_with_address(route: UsbRoute, address: u8) -> UsbRoute {
        if route.parent_hub_address() == 0 || route.parent_port() == 0 {
            UsbRoute::root(address, route.speed())
        } else {
            UsbRoute::via_hub_with_parent_speed(
                address,
                route.speed(),
                route.parent_hub_address(),
                route.parent_port(),
                route.parent_hub_speed(),
            )
        }
    }

    fn enumerate_device(&self, route: UsbRoute, address: u8) -> UsbResult<EnumeratedDevice> {
        let mut header_bytes = [0u8; 8];
        get_descriptor(self, route, 8, USB_DESC_DEVICE, 0, 0, &mut header_bytes)?;
        let header = parse_device_descriptor_header(&header_bytes)?;
        let max_packet_size0 = u16::from(header.max_packet_size0);

        set_address(self, route, 8, address)?;
        Self::delay(Duration::from_millis(10));

        let addressed_route = Self::route_with_address(route, address);

        let mut device_bytes = [0u8; 18];
        get_descriptor(
            self,
            addressed_route,
            max_packet_size0,
            USB_DESC_DEVICE,
            0,
            0,
            &mut device_bytes,
        )?;
        let device_descriptor = parse_device_descriptor(&device_bytes)?;

        let mut configuration_header = [0u8; 9];
        get_descriptor(
            self,
            addressed_route,
            max_packet_size0,
            USB_DESC_CONFIGURATION,
            0,
            0,
            &mut configuration_header,
        )?;
        let configuration = parse_configuration_descriptor(&configuration_header)?;

        let configuration_len = usize::from(configuration.total_length);
        let mut configuration_bytes = vec![0u8; configuration_len];
        get_descriptor(
            self,
            addressed_route,
            max_packet_size0,
            USB_DESC_CONFIGURATION,
            0,
            0,
            &mut configuration_bytes,
        )?;

        set_configuration(
            self,
            addressed_route,
            max_packet_size0,
            configuration.configuration_value,
        )?;
        Self::delay(Duration::from_millis(10));

        Ok(EnumeratedDevice {
            route: addressed_route,
            max_packet_size0,
            device_descriptor,
            configuration_bytes,
        })
    }

    fn read_hub_descriptor(
        &self,
        route: UsbRoute,
        max_packet_size0: u16,
    ) -> UsbResult<HubDescriptorInfo> {
        let mut bytes = [0u8; 16];
        let len = control_in(
            self,
            route,
            max_packet_size0,
            USB_REQ_TYPE_CLASS | USB_REQ_RECIP_DEVICE,
            USB_REQ_GET_DESCRIPTOR,
            u16::from(USB_DESC_HUB) << 8,
            0,
            &mut bytes,
        )?;
        if len < 7 || bytes[1] != USB_DESC_HUB {
            return Err(UsbError::InvalidDescriptor);
        }

        Ok(HubDescriptorInfo {
            num_ports: bytes[2],
            power_on_to_power_good_ms: u64::from(bytes[5]) * 2,
        })
    }

    fn get_hub_port_status(
        &self,
        route: UsbRoute,
        max_packet_size0: u16,
        port: u8,
    ) -> UsbResult<HubPortStatus> {
        let mut status_bytes = [0u8; 4];
        get_status(
            self,
            route,
            max_packet_size0,
            USB_REQ_TYPE_CLASS | USB_REQ_RECIP_OTHER,
            u16::from(port),
            &mut status_bytes,
        )?;

        Ok(HubPortStatus {
            status: u16::from_le_bytes([status_bytes[0], status_bytes[1]]),
            change: u16::from_le_bytes([status_bytes[2], status_bytes[3]]),
        })
    }

    fn clear_hub_port_changes(
        &self,
        route: UsbRoute,
        max_packet_size0: u16,
        port: u8,
        change: u16,
    ) -> UsbResult<()> {
        if (change & HUB_PORT_CHANGE_CONNECTION) != 0 {
            clear_feature(
                self,
                route,
                max_packet_size0,
                USB_REQ_TYPE_CLASS | USB_REQ_RECIP_OTHER,
                HUB_FEATURE_C_PORT_CONNECTION,
                u16::from(port),
            )?;
        }

        if (change & HUB_PORT_CHANGE_ENABLE) != 0 {
            clear_feature(
                self,
                route,
                max_packet_size0,
                USB_REQ_TYPE_CLASS | USB_REQ_RECIP_OTHER,
                HUB_FEATURE_C_PORT_ENABLE,
                u16::from(port),
            )?;
        }

        if (change & HUB_PORT_CHANGE_RESET) != 0 {
            clear_feature(
                self,
                route,
                max_packet_size0,
                USB_REQ_TYPE_CLASS | USB_REQ_RECIP_OTHER,
                HUB_FEATURE_C_PORT_RESET,
                u16::from(port),
            )?;
        }

        Ok(())
    }

    fn power_hub_ports(
        &self,
        route: UsbRoute,
        max_packet_size0: u16,
        descriptor: &HubDescriptorInfo,
    ) -> UsbResult<()> {
        for port in 1..=descriptor.num_ports {
            set_feature(
                self,
                route,
                max_packet_size0,
                USB_REQ_TYPE_CLASS | USB_REQ_RECIP_OTHER,
                USB_HUB_PORT_POWER,
                u16::from(port),
            )?;
        }

        if descriptor.power_on_to_power_good_ms > 0 {
            Self::delay(Duration::from_millis(descriptor.power_on_to_power_good_ms));
        }

        Ok(())
    }

    fn hub_port_speed(status: u16) -> UsbSpeed {
        if (status & HUB_PORT_STATUS_HIGH_SPEED) != 0 {
            UsbSpeed::High
        } else if (status & HUB_PORT_STATUS_LOW_SPEED) != 0 {
            UsbSpeed::Low
        } else {
            UsbSpeed::Full
        }
    }

    fn find_keyboard_interface(descriptors: &[u8]) -> Option<(InterfaceDescriptor, EndpointInfo)> {
        let interface = find_interface(
            descriptors,
            USB_CLASS_HID,
            USB_HID_SUBCLASS_BOOT,
            USB_HID_PROTOCOL_KEYBOARD,
        )?;
        let endpoint = find_interrupt_in_endpoint(descriptors, interface.interface_number)?;
        Some((interface, endpoint))
    }

    fn log_enumerated_device(&self, label: &str, device: &EnumeratedDevice) {
        kprintln!(
            "[INFO][{}] USB {} device: addr={}, speed={:?}, class={:#04x}, subclass={:#04x}, protocol={:#04x}, vendor={:#06x}, product={:#06x}",
            self.id(),
            label,
            device.route.address(),
            device.route.speed(),
            device.device_descriptor.device_class,
            device.device_descriptor.device_subclass,
            device.device_descriptor.device_protocol,
            device.device_descriptor.vendor_id,
            device.device_descriptor.product_id
        );
    }

    fn log_hub_port_status(&self, port: u8, phase: &str, status: &HubPortStatus) {
        kprintln!(
            "[INFO][{}] USB hub port {} {} status={:#06x}, change={:#06x}",
            self.id(),
            port,
            phase,
            status.status,
            status.change
        );
    }

    fn register_keyboard(
        &self,
        controller: Arc<Bcm2708UsbDevice>,
        device: &EnumeratedDevice,
        interface: InterfaceDescriptor,
        endpoint: EndpointInfo,
    ) -> UsbResult<()> {
        if interface.alternate_setting != 0 {
            set_interface(
                self,
                device.route,
                device.max_packet_size0,
                interface.interface_number,
                interface.alternate_setting,
            )?;
        }

        set_protocol(
            self,
            device.route,
            device.max_packet_size0,
            interface.interface_number,
            USB_HID_PROTOCOL_BOOT,
        )?;

        // Request periodic idle reporting (~100 ms = 25 * 4 ms units) so the keyboard
        // re-reports its current state even without a change. This makes a lost key-up edge
        // self-heal within one idle period instead of relying on a single transient report.
        // Not all devices support SET_IDLE; a STALL is harmless, so ignore failures.
        let _ = set_idle(
            self,
            device.route,
            device.max_packet_size0,
            interface.interface_number,
            25,
            0,
        );

        let keyboard = Arc::new(UsbKeyboardDevice::new(
            format!("{}:keyboard@{}", self.id(), device.route.address()),
            controller,
        ));
        kprintln!(
            "[INFO][{}] registering USB boot keyboard: addr={}, interface={}, endpoint={:#04x}, interval={}ms",
            self.id(),
            device.route.address(),
            interface.interface_number,
            endpoint.address.raw(),
            endpoint.interval
        );
        let transfer = InterruptInTransfer {
            route: device.route,
            endpoint,
            expected_len: KEYBOARD_REPORT_LEN,
            buffer: UsbDmaBuffer::new_zeroed(KEYBOARD_REPORT_LEN)?,
            handler: keyboard.clone(),
        };

        self.submit_interrupt_in(transfer)?;
        register_input(keyboard);
        Ok(())
    }

    fn enumerate_keyboard_on_hub_port(
        &self,
        controller: &Arc<Bcm2708UsbDevice>,
        hub: &EnumeratedDevice,
        port: u8,
        next_address: &mut u8,
    ) -> UsbResult<bool> {
        let initial_status = self.get_hub_port_status(hub.route, hub.max_packet_size0, port)?;
        self.log_hub_port_status(port, "initial", &initial_status);
        self.clear_hub_port_changes(hub.route, hub.max_packet_size0, port, initial_status.change)?;

        if (initial_status.status & HUB_PORT_STATUS_CONNECTION) == 0 {
            return Ok(false);
        }

        set_feature(
            self,
            hub.route,
            hub.max_packet_size0,
            USB_REQ_TYPE_CLASS | USB_REQ_RECIP_OTHER,
            USB_HUB_PORT_RESET,
            u16::from(port),
        )?;
        Self::delay(Duration::from_millis(60));

        let port_status = self.get_hub_port_status(hub.route, hub.max_packet_size0, port)?;
        self.log_hub_port_status(port, "post-reset", &port_status);
        self.clear_hub_port_changes(hub.route, hub.max_packet_size0, port, port_status.change)?;
        if (port_status.status & HUB_PORT_STATUS_CONNECTION) == 0
            || (port_status.status & HUB_PORT_STATUS_ENABLED) == 0
            || (port_status.status & HUB_PORT_STATUS_POWERED) == 0
        {
            return Ok(false);
        }

        let route = UsbRoute::via_hub_with_parent_speed(
            0,
            Self::hub_port_speed(port_status.status),
            hub.route.address(),
            port,
            hub.route.speed(),
        );
        let child = self.enumerate_device(route, *next_address)?;
        *next_address = next_address.saturating_add(1);
        self.log_enumerated_device("hub-port", &child);

        if let Some((interface, endpoint)) =
            Self::find_keyboard_interface(&child.configuration_bytes)
        {
            self.register_keyboard(controller.clone(), &child, interface, endpoint)?;
            return Ok(true);
        }

        kprintln!(
            "[INFO][{}] USB hub port {} device is not a HID boot keyboard",
            self.id(),
            port
        );
        Ok(false)
    }

    fn enumerate_boot_keyboard(self: &Arc<Self>) -> UsbResult<bool> {
        if !self.root_port_connected() {
            kprintln!(
                "[INFO][{}] USB root port has no connected device",
                self.id()
            );
            return Ok(false);
        }

        let root_speed = self.reset_root_port()?;
        kprintln!(
            "[INFO][{}] USB root port reset complete: speed={:?}",
            self.id(),
            root_speed
        );
        let mut next_address = 1u8;
        let root = self.enumerate_device(UsbRoute::root(0, root_speed), next_address)?;
        next_address = next_address.saturating_add(1);
        self.log_enumerated_device("root", &root);

        if let Some((interface, endpoint)) =
            Self::find_keyboard_interface(&root.configuration_bytes)
        {
            self.register_keyboard(self.clone(), &root, interface, endpoint)?;
            return Ok(true);
        }

        let hub_like_device = root.device_descriptor.device_class == USB_CLASS_HUB
            || find_interface(&root.configuration_bytes, USB_CLASS_HUB, 0, 0).is_some();
        if !hub_like_device {
            kprintln!(
                "[INFO][{}] USB root device is not a hub or HID boot keyboard",
                self.id()
            );
            return Ok(false);
        }

        let hub_descriptor = self.read_hub_descriptor(root.route, root.max_packet_size0)?;
        kprintln!(
            "[INFO][{}] USB hub detected: ports={}, power-good={}ms",
            self.id(),
            hub_descriptor.num_ports,
            hub_descriptor.power_on_to_power_good_ms
        );
        self.power_hub_ports(root.route, root.max_packet_size0, &hub_descriptor)?;

        for port in 1..=hub_descriptor.num_ports {
            if self.enumerate_keyboard_on_hub_port(self, &root, port, &mut next_address)? {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

impl Device for Bcm2708UsbDevice {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, _node: &Node) -> Result<(), DriverInitError> {
        self.power_on_hcd().map_err(|_| DriverInitError::Retry)?;

        let channel_count = self.init_core().map_err(|error| {
            kprintln!("[ERROR][{}] USB core init failed: {:?}", self.id(), error);
            DriverInitError::DeviceFailed
        })?;
        self.channel_count
            .set(channel_count)
            .map_err(|_| DriverInitError::DeviceFailed)?;

        self.init_host().map_err(|error| {
            kprintln!("[ERROR][{}] USB host init failed: {:?}", self.id(), error);
            DriverInitError::DeviceFailed
        })?;

        register_handler(self.virq, self.clone()).map_err(|_| DriverInitError::Retry)?;
        enable_irq(self.virq).map_err(|_| DriverInitError::Retry)?;
        register_host_controller(self.clone()).map_err(|_| DriverInitError::DeviceFailed)?;

        match self.enumerate_boot_keyboard() {
            Ok(true) => kprintln!("[INFO][{}] boot keyboard detected", self.id()),
            Ok(false) => kprintln!("[INFO][{}] no boot keyboard detected", self.id()),
            Err(error) => kprintln!(
                "[WARNING][{}] boot keyboard bring-up failed: {:?}",
                self.id(),
                error
            ),
        }

        kprintln!(
            "[INFO][{}] initialized USB host controller with {} channels",
            self.id(),
            channel_count
        );

        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl InterruptHandler for Bcm2708UsbDevice {
    fn handle_irq(&self, _virq: u32) {
        let pending =
            self.read_reg(USB_CORE_INT_STATUS_OFFSET) & self.read_reg(USB_CORE_INT_MASK_OFFSET);
        if pending == 0 {
            return;
        }

        if (pending & GINTSTS_PRTINT) != 0 {
            self.handle_port_interrupt();
        }

        if (pending & GINTSTS_DISCONNINT) != 0 {
            self.stop_interrupt_with_error(UsbError::Disconnected);
        }

        if (pending & GINTSTS_HCHINT) != 0
            && (self.read_reg(USB_HOST_ALL_CHANNEL_INT_OFFSET) & (1 << INTERRUPT_CHANNEL)) != 0
        {
            self.handle_interrupt_channel_irq();
        }

        self.write_reg(
            USB_CORE_INT_STATUS_OFFSET,
            pending & (GINTSTS_PRTINT | GINTSTS_HCHINT | GINTSTS_DISCONNINT),
        );
    }
}

impl UsbHostController for Bcm2708UsbDevice {
    fn root_port_connected(&self) -> bool {
        (self.read_reg(USB_HOST_PORT_OFFSET) & HPRT_CONNECT) != 0
    }

    fn reset_root_port(&self) -> UsbResult<UsbSpeed> {
        self.update_host_port(HPRT_POWER, 0, 0);
        if !self.root_port_connected() {
            return Err(UsbError::Disconnected);
        }

        Self::delay(PORT_CONNECT_SETTLE);
        self.update_host_port(HPRT_POWER | HPRT_RESET, 0, 0);
        Self::delay(PORT_RESET_DURATION);
        self.update_host_port(HPRT_POWER, HPRT_RESET, 0);
        self.wait_for_bits(USB_HOST_PORT_OFFSET, HPRT_ENABLE, true, PORT_ENABLE_TIMEOUT)?;
        Self::delay(PORT_RESET_RECOVERY);
        self.acknowledge_host_port_changes();
        self.map_port_speed()
    }

    fn control_transfer(
        &self,
        route: UsbRoute,
        max_packet_size: u16,
        setup: &SetupPacket,
        data: ControlTransferData<'_>,
    ) -> UsbResult<usize> {
        let mut control = self.control.lock();
        let setup_bytes = setup.encode();
        control.setup_buffer.write(&setup_bytes)?;
        control.setup_buffer.clean_cache();

        let data_len = data.len();
        if data_len > control.data_buffer.len() {
            return Err(UsbError::BufferTooLarge);
        }

        let mut input_target = None;
        match data {
            ControlTransferData::None => {}
            ControlTransferData::Out(bytes) => {
                control.data_buffer.write(bytes)?;
                control.data_buffer.clean_cache();
            }
            ControlTransferData::In(bytes) => {
                control.data_buffer.clear();
                control.data_buffer.clean_cache();
                input_target = Some(bytes);
            }
        }

        let setup_dma = Self::bus_address(control.setup_buffer.phys_addr())?;
        self.transfer_packet_polling(
            CONTROL_CHANNEL,
            route,
            0,
            UsbEndpointType::Control,
            UsbDirection::Out,
            max_packet_size,
            PacketId::Setup,
            setup_bytes.len(),
            setup_dma,
        )?;

        let mut transferred = 0usize;
        if data_len > 0 {
            let direction = setup.direction();
            let mut offset = 0usize;
            let mut remaining = data_len;
            let mut packet_id = PacketId::Data1;

            while remaining > 0 {
                let packet_len = min(remaining, usize::from(max_packet_size));
                let packet_dma = Self::bus_address(control.data_buffer.phys_addr() + offset)?;
                let actual = self.transfer_packet_polling(
                    CONTROL_CHANNEL,
                    route,
                    0,
                    UsbEndpointType::Control,
                    direction,
                    max_packet_size,
                    packet_id,
                    packet_len,
                    packet_dma,
                )?;

                transferred += actual;
                offset += actual;
                remaining -= actual;
                if actual < packet_len {
                    break;
                }
                packet_id = packet_id.toggled();
            }
        }

        let status_direction = if setup.direction() == UsbDirection::In {
            UsbDirection::Out
        } else {
            UsbDirection::In
        };
        let status_dma = Self::bus_address(control.setup_buffer.phys_addr())?;
        self.transfer_packet_polling(
            CONTROL_CHANNEL,
            route,
            0,
            UsbEndpointType::Control,
            status_direction,
            max_packet_size,
            PacketId::Data1,
            0,
            status_dma,
        )?;

        if let Some(output) = input_target {
            control.data_buffer.invalidate_cache();
            output[..transferred].copy_from_slice(&control.data_buffer.as_slice()[..transferred]);
        }

        Ok(transferred)
    }

    fn submit_interrupt_in(&self, transfer: InterruptInTransfer) -> UsbResult<()> {
        transfer.validate()?;

        let mut interrupt = self.interrupt.lock_irq();
        if interrupt.is_some() {
            return Err(UsbError::Busy);
        }

        let interval_ticks = Self::interrupt_interval_ticks(transfer.route, transfer.endpoint);
        let mut state = AsyncInterruptState::new(transfer, interval_ticks, Self::read_counter());
        self.start_interrupt_locked(&mut state)?;
        interrupt.replace(state);
        Ok(())
    }

    fn cancel_interrupt_in(&self, route: UsbRoute, endpoint: EndpointAddress) -> UsbResult<()> {
        let mut interrupt = self.interrupt.lock_irq();
        match interrupt.as_ref() {
            Some(state)
                if state.transfer.route == route && state.transfer.endpoint.address == endpoint =>
            {
                self.halt_channel(INTERRUPT_CHANNEL).ok();
                self.configure_channel_irq(INTERRUPT_CHANNEL, 0);
                interrupt.take();
                Ok(())
            }
            _ => Err(UsbError::InvalidState),
        }
    }

    fn poll_interrupt_transfers(&self) {
        self.service_interrupt_polls();
    }
}

pub struct Bcm2708UsbDriver {
    dev_registry: DriverRegistry<Bcm2708UsbDevice>,
}

impl Bcm2708UsbDriver {
    pub const fn new() -> Self {
        Self {
            dev_registry: DriverRegistry::new(),
        }
    }
}

impl PlatformDriver for Bcm2708UsbDriver {
    fn compatible(&self) -> &[&str] {
        &["brcm,bcm2708-usb"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        let _ = get_rpi_firmware().ok_or(DriverInitError::Retry)?;
        let (phys_addr, length) = node
            .resolve_phys_address_and_length(0)
            .ok_or(DriverInitError::DeviceTreeError)?;
        let virq = resolve_virq(node, 0).map_err(|_| DriverInitError::Retry)?;

        let reg_base = map_io_region(phys_addr, length);
        let id = node.path();
        let dev = Bcm2708UsbDevice::new(id.clone(), reg_base, virq)
            .map(Arc::new)
            .map_err(|_| DriverInitError::DeviceFailed)?;

        dev.clone().global_setup(node)?;
        self.dev_registry.add_device(id, dev);
        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

pub static DRIVER: Bcm2708UsbDriver = Bcm2708UsbDriver::new();
