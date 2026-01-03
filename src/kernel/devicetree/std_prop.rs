// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use super::PHandle;
use crate::kernel::devicetree::std_prop::StdPropError::InvalidStatusValue;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const COMPATIBLE: &'static str = "compatible";
pub const MODEL: &'static str = "model";
pub const PHANDLE: &'static str = "phandle";
pub const STATUS: &'static str = "status";
pub const ADDRESS_CELLS: &'static str = "#address-cells";
pub const SIZE_CELLS: &'static str = "#size-cells";
pub const REG: &'static str = "reg";
pub const VIRTUAL_REG: &'static str = "virtual-reg";
pub const RANGES: &'static str = "ranges";
pub const DMA_RANGES: &'static str = "dma-ranges";
pub const DMA_COHERENT: &'static str = "dma-coherent";
pub const DMA_NONCOHERENT: &'static str = "dma-noncoherent";

#[derive(Debug)]
pub enum StdPropError {
    InvalidStatusValue,
    RegConversionError,
}

pub type Result<T> = core::result::Result<T, StdPropError>;

pub trait StandardProperties {
    fn compatible(&self) -> Option<&CompatibleValue>;
    fn model(&self) -> Option<&str>;
    fn phandle(&self) -> Option<&PHandle>;
    fn status(&self) -> Option<&StatusValue>;
    fn address_cells(&self) -> AddressCellsValue;
    fn size_cells(&self) -> SizeCellsValue;
    fn reg(&self) -> Option<&RegValue>;
    fn virtual_reg(&self) -> Option<u32>;
    fn ranges(&self) -> Option<&RangesValue>;
    fn dma_ranges(&self) -> Option<&DmaRangesValue>;
    fn dma_coherent(&self) -> bool;
    fn dma_noncoherent(&self) -> bool;
}

pub type CompatibleValue = Vec<String>;

#[derive(Debug)]
pub enum StatusValue {
    Okay,
    Disabled,
    Reserved,
    Fail(String),
}

impl TryInto<StatusValue> for &str {
    type Error = StdPropError;

    fn try_into(self) -> Result<StatusValue> {
        match self {
            "okay" => Ok(StatusValue::Okay),
            "disabled" => Ok(StatusValue::Disabled),
            "reserved" => Ok(StatusValue::Reserved),
            s if s.starts_with("fail") => Ok(StatusValue::Fail(s.to_string())),
            _ => Err(InvalidStatusValue),
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct AddressCellsValue(pub u32);

impl Default for AddressCellsValue {
    fn default() -> Self {
        Self(2)
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct SizeCellsValue(pub u32);

impl Default for SizeCellsValue {
    fn default() -> Self {
        Self(1)
    }
}

pub type RegValue = Vec<RegItemValue>;

#[derive(Debug)]
pub struct RegItemValue {
    address: Vec<u32>,
    // SAFETY: if #size-cells is 0, the vector will be empty
    length: Vec<u32>,
}

impl RegItemValue {
    pub fn new(address: Vec<u32>, length: Vec<u32>) -> Self {
        Self { address, length }
    }

    pub fn address(&self) -> &[u32] {
        &self.address
    }

    pub fn length(&self) -> &[u32] {
        &self.length
    }

    pub fn address_as_u32(&self) -> Result<u32> {
        if self.address.len() != 1 {
            return Err(StdPropError::RegConversionError);
        }

        Ok(self.address[0])
    }

    pub fn address_as_u64(&self) -> Result<u64> {
        if self.address.len() > 2 {
            return Err(StdPropError::RegConversionError);
        }

        let mut addr: u64 = 0;
        for &part in self.address.iter() {
            addr = (addr << 32) | (part as u64);
        }

        Ok(addr)
    }

    pub fn address_as_usize(&self) -> Result<usize> {
        if (self.address.len() * 4) > size_of::<usize>() {
            return Err(StdPropError::RegConversionError);
        }

        let mut addr: usize = 0;
        for &part in self.address.iter() {
            addr = (addr << 32) | (part as usize);
        }

        Ok(addr)
    }

    pub fn length_as_u32(&self) -> Result<u32> {
        if self.length.len() != 1 {
            return Err(StdPropError::RegConversionError);
        }

        Ok(self.length[0])
    }

    pub fn length_as_u64(&self) -> Result<u64> {
        if self.length.len() > 2 {
            return Err(StdPropError::RegConversionError);
        }

        let mut len: u64 = 0;
        for &part in self.length.iter() {
            len = (len << 32) | (part as u64);
        }

        Ok(len)
    }

    pub fn length_as_usize(&self) -> Result<usize> {
        if (self.length.len() * 4) > size_of::<usize>() {
            return Err(StdPropError::RegConversionError);
        }

        let mut len: usize = 0;
        for &part in self.length.iter() {
            len = (len << 32) | (part as usize);
        }

        Ok(len)
    }
}

pub type RangesValue = Vec<RangesItemValue>;

#[derive(Debug)]
pub struct RangesItemValue {
    child_bus_addr: Vec<u32>,
    parent_bus_addr: Vec<u32>,
    length: Vec<u32>,
}

impl RangesItemValue {
    pub fn new(child_bus_addr: Vec<u32>, parent_bus_addr: Vec<u32>, length: Vec<u32>) -> Self {
        Self {
            child_bus_addr,
            parent_bus_addr,
            length,
        }
    }
    
    pub fn child_bus_addr(&self) -> &[u32] {
        &self.child_bus_addr
    }
    
    pub fn parent_bus_addr(&self) -> &[u32] {
        &self.parent_bus_addr
    }
    
    pub fn length(&self) -> &[u32] {
        &self.length
    }

    pub fn child_bus_addr_as_usize(&self) -> Result<usize> {
        if (self.child_bus_addr.len() * 4) > size_of::<usize>() {
            return Err(StdPropError::RegConversionError);
        }

        let mut addr: usize = 0;
        for &part in self.child_bus_addr.iter() {
            addr = (addr << 32) | (part as usize);
        }

        Ok(addr)
    }

    pub fn parent_bus_addr_as_usize(&self) -> Result<usize> {
        if (self.parent_bus_addr.len() * 4) > size_of::<usize>() {
            return Err(StdPropError::RegConversionError);
        }

        let mut addr: usize = 0;
        for &part in self.parent_bus_addr.iter() {
            addr = (addr << 32) | (part as usize);
        }

        Ok(addr)
    }

    pub fn length_as_usize(&self) -> Result<usize> {
        if (self.length.len() * 4) > size_of::<usize>() {
            return Err(StdPropError::RegConversionError);
        }

        let mut addr: usize = 0;
        for &part in self.length.iter() {
            addr = (addr << 32) | (part as usize);
        }

        Ok(addr)
    }
}

pub type DmaRangesValue = Vec<DmaRangesItemValue>;

#[derive(Debug)]
pub struct DmaRangesItemValue {
    child_bus_addr: Vec<u32>,
    parent_bus_addr: Vec<u32>,
    length: Vec<u32>,
}

impl DmaRangesItemValue {
    pub fn new(child_bus_addr: Vec<u32>, parent_bus_addr: Vec<u32>, length: Vec<u32>) -> Self {
        Self {
            child_bus_addr,
            parent_bus_addr,
            length,
        }
    }
}

#[derive(Debug)]
pub enum StandardProperty {
    Compatible(CompatibleValue),
    Model(String),
    PHandle(PHandle),
    Status(StatusValue),
    AddressCells(AddressCellsValue),
    SizeCells(SizeCellsValue),
    Reg(RegValue),
    VirtualReg(u32),
    Ranges(RangesValue),
    DmaRanges(DmaRangesValue),
    DmaCoherent,
    DmaNoncoherent,
    // deprecated properties
    Name(String),
    DeviceType(String),
}

impl StandardProperty {
        pub fn as_str(&self) -> &str {
        match self {
            StandardProperty::Compatible(_) => COMPATIBLE,
            StandardProperty::Model(_) => MODEL,
            StandardProperty::PHandle(_) => PHANDLE,
            StandardProperty::Status(_) => STATUS,
            StandardProperty::AddressCells(_) => ADDRESS_CELLS,
            StandardProperty::SizeCells(_) => SIZE_CELLS,
            StandardProperty::Reg(_) => REG,
            StandardProperty::VirtualReg(_) => VIRTUAL_REG,
            StandardProperty::Ranges(_) => RANGES,
            StandardProperty::DmaRanges(_) => DMA_RANGES,
            StandardProperty::DmaCoherent => DMA_COHERENT,
            StandardProperty::DmaNoncoherent => DMA_NONCOHERENT,
            // deprecated properties
            StandardProperty::Name(_) => "name",
            StandardProperty::DeviceType(_) => "device_type",
        }
    }
}
