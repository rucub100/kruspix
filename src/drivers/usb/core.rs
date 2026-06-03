// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>
//
// @vibe-coded

use alloc::sync::Arc;
use core::cmp::min;
use core::ptr;
use core::slice;

use crate::arch::cpu::{clean_data_cache_range, invalidate_data_cache_range};
use crate::arch::mm::mmu::PAGE_SIZE;
use crate::drivers::Device;
use crate::mm::{alloc_page, dealloc_page, virt_to_phys};

pub const USB_REQ_DIR_OUT: u8 = 0x00;
pub const USB_REQ_DIR_IN: u8 = 0x80;

pub const USB_REQ_TYPE_STANDARD: u8 = 0x00;
pub const USB_REQ_TYPE_CLASS: u8 = 0x20;
pub const USB_REQ_TYPE_VENDOR: u8 = 0x40;

pub const USB_REQ_RECIP_DEVICE: u8 = 0x00;
pub const USB_REQ_RECIP_INTERFACE: u8 = 0x01;
pub const USB_REQ_RECIP_ENDPOINT: u8 = 0x02;
pub const USB_REQ_RECIP_OTHER: u8 = 0x03;

pub const USB_REQ_GET_STATUS: u8 = 0x00;
pub const USB_REQ_CLEAR_FEATURE: u8 = 0x01;
pub const USB_REQ_SET_FEATURE: u8 = 0x03;
pub const USB_REQ_SET_ADDRESS: u8 = 0x05;
pub const USB_REQ_GET_DESCRIPTOR: u8 = 0x06;
pub const USB_REQ_SET_DESCRIPTOR: u8 = 0x07;
pub const USB_REQ_GET_CONFIGURATION: u8 = 0x08;
pub const USB_REQ_SET_CONFIGURATION: u8 = 0x09;
pub const USB_REQ_GET_INTERFACE: u8 = 0x0A;
pub const USB_REQ_SET_INTERFACE: u8 = 0x0B;

pub const USB_DESC_DEVICE: u8 = 0x01;
pub const USB_DESC_CONFIGURATION: u8 = 0x02;
pub const USB_DESC_STRING: u8 = 0x03;
pub const USB_DESC_INTERFACE: u8 = 0x04;
pub const USB_DESC_ENDPOINT: u8 = 0x05;
pub const USB_DESC_HID: u8 = 0x21;
pub const USB_DESC_REPORT: u8 = 0x22;
pub const USB_DESC_HUB: u8 = 0x29;

pub const USB_CLASS_HID: u8 = 0x03;
pub const USB_CLASS_HUB: u8 = 0x09;

pub const USB_HID_SUBCLASS_BOOT: u8 = 0x01;
pub const USB_HID_PROTOCOL_KEYBOARD: u8 = 0x01;

pub const USB_HUB_PORT_RESET: u16 = 0x0004;
pub const USB_HUB_PORT_POWER: u16 = 0x0008;
pub const USB_HID_SET_PROTOCOL: u8 = 0x0B;
pub const USB_HID_SET_IDLE: u8 = 0x0A;
pub const USB_HID_PROTOCOL_BOOT: u16 = 0x0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbError {
    NoMemory,
    BufferTooLarge,
    BufferTooSmall,
    InvalidDescriptor,
    InvalidPacket,
    InvalidEndpoint,
    InvalidState,
    Disconnected,
    Timeout,
    Stall,
    Busy,
    Unsupported,
    ShortPacket,
    HardwareFault,
}

pub type UsbResult<T> = Result<T, UsbError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    Low,
    Full,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbDirection {
    Out,
    In,
}

impl UsbDirection {
    pub const fn from_request_type(request_type: u8) -> Self {
        if (request_type & USB_REQ_DIR_IN) != 0 {
            Self::In
        } else {
            Self::Out
        }
    }

    pub const fn from_endpoint_address(endpoint_address: u8) -> Self {
        if (endpoint_address & USB_REQ_DIR_IN) != 0 {
            Self::In
        } else {
            Self::Out
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbEndpointType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

impl UsbEndpointType {
    pub const fn from_attributes(attributes: u8) -> Self {
        match attributes & 0x03 {
            0x00 => Self::Control,
            0x01 => Self::Isochronous,
            0x02 => Self::Bulk,
            _ => Self::Interrupt,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbRoute {
    address: u8,
    speed: UsbSpeed,
    parent_hub_address: u8,
    parent_port: u8,
    parent_hub_speed: UsbSpeed,
}

impl UsbRoute {
    pub const fn root(address: u8, speed: UsbSpeed) -> Self {
        Self {
            address,
            speed,
            parent_hub_address: 0,
            parent_port: 0,
            parent_hub_speed: UsbSpeed::High,
        }
    }

    pub const fn via_hub(
        address: u8,
        speed: UsbSpeed,
        parent_hub_address: u8,
        parent_port: u8,
    ) -> Self {
        Self::via_hub_with_parent_speed(
            address,
            speed,
            parent_hub_address,
            parent_port,
            UsbSpeed::High,
        )
    }

    pub const fn via_hub_with_parent_speed(
        address: u8,
        speed: UsbSpeed,
        parent_hub_address: u8,
        parent_port: u8,
        parent_hub_speed: UsbSpeed,
    ) -> Self {
        Self {
            address,
            speed,
            parent_hub_address,
            parent_port,
            parent_hub_speed,
        }
    }

    pub const fn address(self) -> u8 {
        self.address
    }

    pub const fn speed(self) -> UsbSpeed {
        self.speed
    }

    pub const fn parent_hub_address(self) -> u8 {
        self.parent_hub_address
    }

    pub const fn parent_port(self) -> u8 {
        self.parent_port
    }

    pub const fn parent_hub_speed(self) -> UsbSpeed {
        self.parent_hub_speed
    }

    pub fn uses_split_transactions(self) -> bool {
        self.parent_hub_address != 0
            && self.parent_port != 0
            && self.parent_hub_speed == UsbSpeed::High
            && self.speed != UsbSpeed::High
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SetupPacket {
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    length: u16,
}

impl SetupPacket {
    pub const fn new(request_type: u8, request: u8, value: u16, index: u16, length: u16) -> Self {
        Self {
            request_type,
            request,
            value,
            index,
            length,
        }
    }

    pub const fn request_type(self) -> u8 {
        self.request_type
    }

    pub const fn request(self) -> u8 {
        self.request
    }

    pub const fn value(self) -> u16 {
        self.value
    }

    pub const fn index(self) -> u16 {
        self.index
    }

    pub const fn length(self) -> u16 {
        self.length
    }

    pub const fn direction(self) -> UsbDirection {
        UsbDirection::from_request_type(self.request_type)
    }

    pub fn encode(self) -> [u8; 8] {
        let value = self.value.to_le_bytes();
        let index = self.index.to_le_bytes();
        let length = self.length.to_le_bytes();

        [
            self.request_type,
            self.request,
            value[0],
            value[1],
            index[0],
            index[1],
            length[0],
            length[1],
        ]
    }
}

pub enum ControlTransferData<'a> {
    None,
    In(&'a mut [u8]),
    Out(&'a [u8]),
}

impl<'a> ControlTransferData<'a> {
    pub fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::In(bytes) => bytes.len(),
            Self::Out(bytes) => bytes.len(),
        }
    }
}

pub struct UsbDmaBuffer {
    virt_addr: usize,
    phys_addr: usize,
    len: usize,
}

impl UsbDmaBuffer {
    pub fn new_zeroed(len: usize) -> UsbResult<Self> {
        if len == 0 {
            return Err(UsbError::BufferTooSmall);
        }

        if len > PAGE_SIZE {
            return Err(UsbError::BufferTooLarge);
        }

        let page = alloc_page();
        if page.is_null() {
            return Err(UsbError::NoMemory);
        }

        // SAFETY: `page` points at an exclusive page-sized allocation returned by `alloc_page()`.
        // Zeroing `PAGE_SIZE` bytes is within that allocation and initializes the transfer buffer.
        unsafe {
            ptr::write_bytes(page, 0, PAGE_SIZE);
        }

        Ok(Self {
            virt_addr: page as usize,
            phys_addr: virt_to_phys(page as usize),
            len,
        })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn virt_addr(&self) -> usize {
        self.virt_addr
    }

    pub const fn phys_addr(&self) -> usize {
        self.phys_addr
    }

    pub fn phys_addr_u32(&self) -> UsbResult<u32> {
        u32::try_from(self.phys_addr).map_err(|_| UsbError::Unsupported)
    }

    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: `virt_addr` points at a live page allocation owned by this buffer for at least
        // `len` bytes. The returned slice borrows `self`, so it cannot outlive the allocation.
        unsafe { slice::from_raw_parts(self.virt_addr as *const u8, self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: `virt_addr` points at a live page allocation owned exclusively by this buffer for
        // at least `len` bytes. `&mut self` guarantees exclusive access to the slice contents.
        unsafe { slice::from_raw_parts_mut(self.virt_addr as *mut u8, self.len) }
    }

    pub fn clear(&mut self) {
        self.as_mut_slice().fill(0);
    }

    pub fn write(&mut self, bytes: &[u8]) -> UsbResult<()> {
        if bytes.len() > self.len {
            return Err(UsbError::BufferTooLarge);
        }

        let slice = self.as_mut_slice();
        slice[..bytes.len()].copy_from_slice(bytes);
        slice[bytes.len()..].fill(0);
        Ok(())
    }

    pub fn clean_cache(&self) {
        // SAFETY: `virt_addr..virt_addr+len` names the live page-backed buffer owned by `self`.
        unsafe {
            clean_data_cache_range(self.virt_addr, self.len);
        }
    }

    pub fn invalidate_cache(&self) {
        // SAFETY: `virt_addr..virt_addr+len` names the live page-backed buffer owned by `self`.
        unsafe {
            invalidate_data_cache_range(self.virt_addr, self.len);
        }
    }
}

impl Drop for UsbDmaBuffer {
    fn drop(&mut self) {
        dealloc_page(self.virt_addr as *mut u8);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointAddress(u8);

impl EndpointAddress {
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u8 {
        self.0
    }

    pub const fn number(self) -> u8 {
        self.0 & 0x0F
    }

    pub const fn direction(self) -> UsbDirection {
        UsbDirection::from_endpoint_address(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointInfo {
    pub address: EndpointAddress,
    pub attributes: u8,
    pub max_packet_size: u16,
    pub interval: u8,
}

impl EndpointInfo {
    pub const fn endpoint_type(self) -> UsbEndpointType {
        UsbEndpointType::from_attributes(self.attributes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceDescriptorHeader {
    pub usb_version: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub max_packet_size0: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceDescriptor {
    pub usb_version: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub max_packet_size0: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_version: u16,
    pub manufacturer_index: u8,
    pub product_index: u8,
    pub serial_number_index: u8,
    pub num_configurations: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigurationDescriptor {
    pub total_length: u16,
    pub num_interfaces: u8,
    pub configuration_value: u8,
    pub configuration_index: u8,
    pub attributes: u8,
    pub max_power: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceDescriptor {
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub num_endpoints: u8,
    pub interface_class: u8,
    pub interface_subclass: u8,
    pub interface_protocol: u8,
    pub interface_index: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidDescriptor {
    pub hid_version: u16,
    pub country_code: u8,
    pub num_descriptors: u8,
    pub report_descriptor_type: u8,
    pub report_descriptor_length: u16,
}

#[derive(Clone, Copy)]
pub struct RawDescriptor<'a> {
    bytes: &'a [u8],
}

impl<'a> RawDescriptor<'a> {
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    pub const fn length(self) -> usize {
        self.bytes.len()
    }

    pub fn descriptor_type(self) -> u8 {
        self.bytes.get(1).copied().unwrap_or(0)
    }

    pub fn as_device_header(self) -> UsbResult<DeviceDescriptorHeader> {
        parse_device_descriptor_header(self.bytes)
    }

    pub fn as_device(self) -> UsbResult<DeviceDescriptor> {
        parse_device_descriptor(self.bytes)
    }

    pub fn as_configuration(self) -> UsbResult<ConfigurationDescriptor> {
        parse_configuration_descriptor(self.bytes)
    }

    pub fn as_interface(self) -> UsbResult<InterfaceDescriptor> {
        parse_interface_descriptor(self.bytes)
    }

    pub fn as_endpoint(self) -> UsbResult<EndpointInfo> {
        parse_endpoint_descriptor(self.bytes)
    }

    pub fn as_hid(self) -> UsbResult<HidDescriptor> {
        parse_hid_descriptor(self.bytes)
    }
}

pub struct DescriptorIter<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> DescriptorIter<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
}

impl<'a> Iterator for DescriptorIter<'a> {
    type Item = RawDescriptor<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset + 2 > self.bytes.len() {
            return None;
        }

        let length = self.bytes[self.offset] as usize;
        if length < 2 || self.offset + length > self.bytes.len() {
            self.offset = self.bytes.len();
            return None;
        }

        let descriptor = RawDescriptor {
            bytes: &self.bytes[self.offset..self.offset + length],
        };
        self.offset += length;
        Some(descriptor)
    }
}

pub trait InterruptInHandler: Send + Sync {
    fn handle_interrupt_in(&self, data: &[u8]);

    fn handle_interrupt_error(&self, _error: UsbError) {}
}

pub struct InterruptInTransfer {
    pub route: UsbRoute,
    pub endpoint: EndpointInfo,
    pub expected_len: usize,
    pub buffer: UsbDmaBuffer,
    pub handler: Arc<dyn InterruptInHandler>,
}

impl InterruptInTransfer {
    pub fn validate(&self) -> UsbResult<()> {
        if self.endpoint.address.direction() != UsbDirection::In {
            return Err(UsbError::InvalidEndpoint);
        }

        if self.endpoint.endpoint_type() != UsbEndpointType::Interrupt {
            return Err(UsbError::InvalidEndpoint);
        }

        if self.expected_len == 0 || self.expected_len > self.buffer.len() {
            return Err(UsbError::BufferTooSmall);
        }

        Ok(())
    }
}

pub trait UsbHostController: Device {
    fn root_port_connected(&self) -> bool;
    fn reset_root_port(&self) -> UsbResult<UsbSpeed>;
    fn control_transfer(
        &self,
        route: UsbRoute,
        max_packet_size: u16,
        setup: &SetupPacket,
        data: ControlTransferData<'_>,
    ) -> UsbResult<usize>;
    fn submit_interrupt_in(&self, transfer: InterruptInTransfer) -> UsbResult<()>;
    fn cancel_interrupt_in(&self, _route: UsbRoute, _endpoint: EndpointAddress) -> UsbResult<()> {
        Err(UsbError::Unsupported)
    }
    /// Drive any background interrupt transfers (re-arm due polls, run the in-flight watchdog).
    /// Intended to be called periodically from a dedicated task so servicing is not coupled to
    /// the input-consuming loop. Default is a no-op for controllers that do not need it.
    fn poll_interrupt_transfers(&self) {}
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let raw = bytes.get(offset..end)?;
    Some(u16::from_le_bytes([raw[0], raw[1]]))
}

pub fn parse_device_descriptor_header(bytes: &[u8]) -> UsbResult<DeviceDescriptorHeader> {
    if bytes.len() < 8 || bytes[0] < 8 || bytes[1] != USB_DESC_DEVICE {
        return Err(UsbError::InvalidDescriptor);
    }

    Ok(DeviceDescriptorHeader {
        usb_version: read_u16_le(bytes, 2).ok_or(UsbError::InvalidDescriptor)?,
        device_class: *bytes.get(4).ok_or(UsbError::InvalidDescriptor)?,
        device_subclass: *bytes.get(5).ok_or(UsbError::InvalidDescriptor)?,
        device_protocol: *bytes.get(6).ok_or(UsbError::InvalidDescriptor)?,
        max_packet_size0: *bytes.get(7).ok_or(UsbError::InvalidDescriptor)?,
    })
}

pub fn parse_device_descriptor(bytes: &[u8]) -> UsbResult<DeviceDescriptor> {
    if bytes.len() < 18 || bytes[0] < 18 || bytes[1] != USB_DESC_DEVICE {
        return Err(UsbError::InvalidDescriptor);
    }

    Ok(DeviceDescriptor {
        usb_version: read_u16_le(bytes, 2).ok_or(UsbError::InvalidDescriptor)?,
        device_class: *bytes.get(4).ok_or(UsbError::InvalidDescriptor)?,
        device_subclass: *bytes.get(5).ok_or(UsbError::InvalidDescriptor)?,
        device_protocol: *bytes.get(6).ok_or(UsbError::InvalidDescriptor)?,
        max_packet_size0: *bytes.get(7).ok_or(UsbError::InvalidDescriptor)?,
        vendor_id: read_u16_le(bytes, 8).ok_or(UsbError::InvalidDescriptor)?,
        product_id: read_u16_le(bytes, 10).ok_or(UsbError::InvalidDescriptor)?,
        device_version: read_u16_le(bytes, 12).ok_or(UsbError::InvalidDescriptor)?,
        manufacturer_index: *bytes.get(14).ok_or(UsbError::InvalidDescriptor)?,
        product_index: *bytes.get(15).ok_or(UsbError::InvalidDescriptor)?,
        serial_number_index: *bytes.get(16).ok_or(UsbError::InvalidDescriptor)?,
        num_configurations: *bytes.get(17).ok_or(UsbError::InvalidDescriptor)?,
    })
}

pub fn parse_configuration_descriptor(bytes: &[u8]) -> UsbResult<ConfigurationDescriptor> {
    if bytes.len() < 9 || bytes[0] < 9 || bytes[1] != USB_DESC_CONFIGURATION {
        return Err(UsbError::InvalidDescriptor);
    }

    Ok(ConfigurationDescriptor {
        total_length: read_u16_le(bytes, 2).ok_or(UsbError::InvalidDescriptor)?,
        num_interfaces: *bytes.get(4).ok_or(UsbError::InvalidDescriptor)?,
        configuration_value: *bytes.get(5).ok_or(UsbError::InvalidDescriptor)?,
        configuration_index: *bytes.get(6).ok_or(UsbError::InvalidDescriptor)?,
        attributes: *bytes.get(7).ok_or(UsbError::InvalidDescriptor)?,
        max_power: *bytes.get(8).ok_or(UsbError::InvalidDescriptor)?,
    })
}

pub fn parse_interface_descriptor(bytes: &[u8]) -> UsbResult<InterfaceDescriptor> {
    if bytes.len() < 9 || bytes[0] < 9 || bytes[1] != USB_DESC_INTERFACE {
        return Err(UsbError::InvalidDescriptor);
    }

    Ok(InterfaceDescriptor {
        interface_number: *bytes.get(2).ok_or(UsbError::InvalidDescriptor)?,
        alternate_setting: *bytes.get(3).ok_or(UsbError::InvalidDescriptor)?,
        num_endpoints: *bytes.get(4).ok_or(UsbError::InvalidDescriptor)?,
        interface_class: *bytes.get(5).ok_or(UsbError::InvalidDescriptor)?,
        interface_subclass: *bytes.get(6).ok_or(UsbError::InvalidDescriptor)?,
        interface_protocol: *bytes.get(7).ok_or(UsbError::InvalidDescriptor)?,
        interface_index: *bytes.get(8).ok_or(UsbError::InvalidDescriptor)?,
    })
}

pub fn parse_endpoint_descriptor(bytes: &[u8]) -> UsbResult<EndpointInfo> {
    if bytes.len() < 7 || bytes[0] < 7 || bytes[1] != USB_DESC_ENDPOINT {
        return Err(UsbError::InvalidDescriptor);
    }

    Ok(EndpointInfo {
        address: EndpointAddress::new(*bytes.get(2).ok_or(UsbError::InvalidDescriptor)?),
        attributes: *bytes.get(3).ok_or(UsbError::InvalidDescriptor)?,
        max_packet_size: read_u16_le(bytes, 4).ok_or(UsbError::InvalidDescriptor)?,
        interval: *bytes.get(6).ok_or(UsbError::InvalidDescriptor)?,
    })
}

pub fn parse_hid_descriptor(bytes: &[u8]) -> UsbResult<HidDescriptor> {
    if bytes.len() < 9 || bytes[0] < 9 || bytes[1] != USB_DESC_HID {
        return Err(UsbError::InvalidDescriptor);
    }

    Ok(HidDescriptor {
        hid_version: read_u16_le(bytes, 2).ok_or(UsbError::InvalidDescriptor)?,
        country_code: *bytes.get(4).ok_or(UsbError::InvalidDescriptor)?,
        num_descriptors: *bytes.get(5).ok_or(UsbError::InvalidDescriptor)?,
        report_descriptor_type: *bytes.get(6).ok_or(UsbError::InvalidDescriptor)?,
        report_descriptor_length: read_u16_le(bytes, 7).ok_or(UsbError::InvalidDescriptor)?,
    })
}

pub fn descriptor_iter(bytes: &[u8]) -> DescriptorIter<'_> {
    DescriptorIter::new(bytes)
}

pub fn find_interface(
    descriptors: &[u8],
    interface_class: u8,
    interface_subclass: u8,
    interface_protocol: u8,
) -> Option<InterfaceDescriptor> {
    descriptor_iter(descriptors).find_map(|descriptor| {
        let interface = descriptor.as_interface().ok()?;
        if interface.interface_class == interface_class
            && interface.interface_subclass == interface_subclass
            && interface.interface_protocol == interface_protocol
        {
            Some(interface)
        } else {
            None
        }
    })
}

pub fn find_interrupt_in_endpoint(
    descriptors: &[u8],
    interface_number: u8,
) -> Option<EndpointInfo> {
    let mut active_interface = None;

    for descriptor in descriptor_iter(descriptors) {
        match descriptor.descriptor_type() {
            USB_DESC_INTERFACE => {
                active_interface = descriptor
                    .as_interface()
                    .ok()
                    .map(|interface| interface.interface_number);
            }
            USB_DESC_ENDPOINT if active_interface == Some(interface_number) => {
                let endpoint = descriptor.as_endpoint().ok()?;
                if endpoint.address.direction() == UsbDirection::In
                    && endpoint.endpoint_type() == UsbEndpointType::Interrupt
                {
                    return Some(endpoint);
                }
            }
            _ => {}
        }
    }

    None
}

pub fn control_in(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    buffer: &mut [u8],
) -> UsbResult<usize> {
    let length = min(buffer.len(), u16::MAX as usize) as u16;
    let setup = SetupPacket::new(request_type | USB_REQ_DIR_IN, request, value, index, length);
    controller.control_transfer(
        route,
        max_packet_size,
        &setup,
        ControlTransferData::In(buffer),
    )
}

pub fn control_out(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    buffer: &[u8],
) -> UsbResult<usize> {
    let length = min(buffer.len(), u16::MAX as usize) as u16;
    let setup = SetupPacket::new(
        request_type & !USB_REQ_DIR_IN,
        request,
        value,
        index,
        length,
    );
    controller.control_transfer(
        route,
        max_packet_size,
        &setup,
        ControlTransferData::Out(buffer),
    )
}

pub fn control_no_data(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
) -> UsbResult<()> {
    let setup = SetupPacket::new(request_type & !USB_REQ_DIR_IN, request, value, index, 0);
    controller
        .control_transfer(route, max_packet_size, &setup, ControlTransferData::None)
        .map(|_| ())
}

pub fn get_descriptor(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    descriptor_type: u8,
    descriptor_index: u8,
    language_id: u16,
    buffer: &mut [u8],
) -> UsbResult<usize> {
    let descriptor_value = ((descriptor_type as u16) << 8) | descriptor_index as u16;
    control_in(
        controller,
        route,
        max_packet_size,
        USB_REQ_TYPE_STANDARD | USB_REQ_RECIP_DEVICE,
        USB_REQ_GET_DESCRIPTOR,
        descriptor_value,
        language_id,
        buffer,
    )
}

pub fn set_address(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    address: u8,
) -> UsbResult<()> {
    control_no_data(
        controller,
        route,
        max_packet_size,
        USB_REQ_TYPE_STANDARD | USB_REQ_RECIP_DEVICE,
        USB_REQ_SET_ADDRESS,
        u16::from(address),
        0,
    )
}

pub fn set_configuration(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    configuration_value: u8,
) -> UsbResult<()> {
    control_no_data(
        controller,
        route,
        max_packet_size,
        USB_REQ_TYPE_STANDARD | USB_REQ_RECIP_DEVICE,
        USB_REQ_SET_CONFIGURATION,
        u16::from(configuration_value),
        0,
    )
}

pub fn set_interface(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    interface_number: u8,
    alternate_setting: u8,
) -> UsbResult<()> {
    control_no_data(
        controller,
        route,
        max_packet_size,
        USB_REQ_TYPE_STANDARD | USB_REQ_RECIP_INTERFACE,
        USB_REQ_SET_INTERFACE,
        u16::from(alternate_setting),
        u16::from(interface_number),
    )
}

pub fn set_protocol(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    interface_number: u8,
    protocol: u16,
) -> UsbResult<()> {
    control_no_data(
        controller,
        route,
        max_packet_size,
        USB_REQ_TYPE_CLASS | USB_REQ_RECIP_INTERFACE,
        USB_HID_SET_PROTOCOL,
        protocol,
        u16::from(interface_number),
    )
}

/// Issue a HID SET_IDLE request. `duration_units` is in 4 ms steps (0 = report only on
/// change, non-zero = also re-report the current state every `duration_units * 4` ms).
/// Periodic re-reporting makes a missed key-up edge self-heal: every subsequent poll
/// carries the current (released) state instead of relying on a single transient edge.
pub fn set_idle(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    interface_number: u8,
    duration_units: u8,
    report_id: u8,
) -> UsbResult<()> {
    let value = (u16::from(duration_units) << 8) | u16::from(report_id);
    control_no_data(
        controller,
        route,
        max_packet_size,
        USB_REQ_TYPE_CLASS | USB_REQ_RECIP_INTERFACE,
        USB_HID_SET_IDLE,
        value,
        u16::from(interface_number),
    )
}

pub fn get_status(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    request_type: u8,
    index: u16,
    buffer: &mut [u8; 4],
) -> UsbResult<usize> {
    control_in(
        controller,
        route,
        max_packet_size,
        request_type,
        USB_REQ_GET_STATUS,
        0,
        index,
        buffer,
    )
}

pub fn set_feature(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    request_type: u8,
    feature: u16,
    index: u16,
) -> UsbResult<()> {
    control_no_data(
        controller,
        route,
        max_packet_size,
        request_type,
        USB_REQ_SET_FEATURE,
        feature,
        index,
    )
}

pub fn clear_feature(
    controller: &dyn UsbHostController,
    route: UsbRoute,
    max_packet_size: u16,
    request_type: u8,
    feature: u16,
    index: u16,
) -> UsbResult<()> {
    control_no_data(
        controller,
        route,
        max_packet_size,
        request_type,
        USB_REQ_CLEAR_FEATURE,
        feature,
        index,
    )
}
