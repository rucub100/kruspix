use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::fdt::prop::{Prop, StandardProp};

#[derive(Debug)]
pub enum PropertyValue {
    Standard(StandardProperty),
    Other(PropertyValueType),
}

#[derive(Debug)]
pub struct Property {
    name: String,
    value: PropertyValue,
}

impl Property {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &PropertyValue {
        &self.value
    }
}

impl TryFrom<&Prop> for Property {
    type Error = ();

    fn try_from(prop: &Prop) -> Result<Self, Self::Error> {
        let mut value: PropertyValue =
            PropertyValue::Other(PropertyValueType::PropEncodedArray(prop.value().to_vec()));

        let standard_prop: Result<StandardProp, ()> = prop.name().try_into();
        if let Ok(standard_prop) = standard_prop {
            value = match standard_prop {
                StandardProp::Compatible => PropertyValue::Standard(StandardProperty::Compatible(
                    prop.value_as_string_list_iter()
                        .filter_map(|res| {
                            res.ok()
                                .and_then(|cstr| cstr.to_str().ok())
                                .map(|s| s.to_string())
                        })
                        .collect(),
                )),
                StandardProp::Model => PropertyValue::Standard(StandardProperty::Model(
                    prop.value_as_string()?.to_string(),
                )),
                StandardProp::PHandle => {
                    PropertyValue::Standard(StandardProperty::PHandle(prop.value_as_u32()?))
                }
                StandardProp::Status => PropertyValue::Standard(StandardProperty::Status(
                    prop.value_as_string()?.to_string(),
                )),
                StandardProp::AddressCells => {
                    PropertyValue::Standard(StandardProperty::AddressCells(prop.value_as_u32()?))
                }
                StandardProp::SizeCells => {
                    PropertyValue::Standard(StandardProperty::SizeCells(prop.value_as_u32()?))
                }
                // we cannot parse the value here because we don't know the #address-cells and #size-cells of the parent node
                // StandardProp::Reg => PropertyValue::Standard(StandardProperty::Reg(prop.value_as_prop_encoded_array_cells_pair_iter(/* u32 */, /* u32 */).collect())),
                StandardProp::VirtualReg => PropertyValue::Standard(StandardProperty::VirtualReg(
                    prop.value_as_u32()? as usize,
                )),
                // we cannot parse the value here because we don't know the #address-cells and #size-cells of the parent node
                // StandardProp::Ranges => PropertyValue::Standard(StandardProperty::Ranges(prop.value_as_optional_prop_encoded_array_cells_triple_iter(/* u32 */, /* u32 */, /* u32 */).collect())),
                // StandardProp::DmaRanges => PropertyValue::Standard(StandardProperty::DmaRanges(prop.value_as_optional_prop_encoded_array_cells_triple_iter(/* u32 */, /* u32 */, /* u32 */).collect())),
                StandardProp::DmaCoherent => PropertyValue::Standard(StandardProperty::DmaCoherent),
                StandardProp::DmaNoncoherent => {
                    PropertyValue::Standard(StandardProperty::DmaNoncoherent)
                }
                _ => value,
            }
        }

        Ok(Self {
            name: prop.name().to_string(),
            value,
        })
    }
}

#[derive(Debug)]
pub enum PropertyValueType {
    Empty,
    U32(u32),
    U64(u64),
    String(String),
    PropEncodedArray(Vec<u8>),
    PHandle(u32),
    StringList(Vec<String>),
}

pub enum StatusValue {
    Ok,
    Disabled,
    Reserved,
    Fail(String),
}

#[derive(Debug)]
pub enum StandardProperty {
    Compatible(Vec<String>),
    Model(String),
    PHandle(u32),
    Status(String),
    AddressCells(u32),
    SizeCells(u32),
    Reg(Vec<(usize, usize)>),
    VirtualReg(usize),
    Ranges(Option<Vec<(usize, usize, usize)>>),
    DmaRanges(Option<Vec<(usize, usize, usize)>>),
    DmaCoherent,
    DmaNoncoherent,
    // deprecated properties
    Name(String),
    DeviceType(String),
}

impl StandardProperty {
    pub const COMPATIBLE: &'static str = "compatible";
    pub const MODEL: &'static str = "model";
    pub const P_HANDLE: &'static str = "phandle";
    pub const STATUS: &'static str = "status";
    pub const ADDRESS_CELLS: &'static str = "#address-cells";
    pub const SIZE_CELLS: &'static str = "#size-cells";
    pub const REG: &'static str = "reg";
    pub const VIRTUAL_REG: &'static str = "virtual-reg";
    pub const RANGES: &'static str = "ranges";
    pub const DMA_RANGES: &'static str = "dma-ranges";
    pub const DMA_COHERENT: &'static str = "dma-coherent";
    pub const DMA_NONCOHERENT: &'static str = "dma-noncoherent";

    pub fn name(&self) -> &str {
        match self {
            StandardProperty::Compatible(_) => Self::COMPATIBLE,
            StandardProperty::Model(_) => Self::MODEL,
            StandardProperty::PHandle(_) => Self::P_HANDLE,
            StandardProperty::Status(_) => Self::STATUS,
            StandardProperty::AddressCells(_) => Self::ADDRESS_CELLS,
            StandardProperty::SizeCells(_) => Self::SIZE_CELLS,
            StandardProperty::Reg(_) => Self::REG,
            StandardProperty::VirtualReg(_) => Self::VIRTUAL_REG,
            StandardProperty::Ranges(_) => Self::RANGES,
            StandardProperty::DmaRanges(_) => Self::DMA_RANGES,
            StandardProperty::DmaCoherent => Self::DMA_COHERENT,
            StandardProperty::DmaNoncoherent => Self::DMA_NONCOHERENT,
            // deprecated properties
            StandardProperty::Name(_) => "name",
            StandardProperty::DeviceType(_) => "device_type",
        }
    }
}
