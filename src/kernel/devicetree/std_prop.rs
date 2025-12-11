use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::PHandle;

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
    type Error = ();

    fn try_into(self) -> Result<StatusValue, ()> {
        match self {
            "okay" => Ok(StatusValue::Okay),
            "disabled" => Ok(StatusValue::Disabled),
            "reserved" => Ok(StatusValue::Reserved),
            s if s.starts_with("fail") => Ok(StatusValue::Fail(s.to_string())),
            _ => Err(()),
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
