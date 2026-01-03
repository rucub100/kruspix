// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::CStr;

use super::PHandle;
use super::fdt::raw_prop::RawProp;
use super::interrupts::{
    ExtendedInterrupts, INTERRUPT_CELLS, INTERRUPT_CONTROLLER, INTERRUPT_MAP, INTERRUPT_MAP_MASK,
    INTERRUPT_PARENT, INTERRUPTS, INTERRUPTS_EXTENDED, InterruptMap, InterruptMapMask, Interrupts,
    InterruptsProperty,
};
use super::misc_prop::CLOCK_FREQUENCY;
use super::node::Node;
use super::std_prop::{
    ADDRESS_CELLS, AddressCellsValue, COMPATIBLE, DMA_COHERENT, DMA_NONCOHERENT, DMA_RANGES,
    DmaRangesItemValue, MODEL, PHANDLE, RANGES, REG, RangesItemValue, RegItemValue, SIZE_CELLS,
    STATUS, SizeCellsValue, StandardProperties, StandardProperty, VIRTUAL_REG,
};
use crate::kernel::devicetree::misc_prop::{ClockFrequency, MiscellaneousProperty};

#[derive(Debug)]
pub struct Property {
    name: String,
    value: PropertyValue,
}

impl Property {
    pub fn from_raw(prop: &RawProp, node: &Node) -> Self {
        let value = match prop.name() {
            // Standard Properties
            COMPATIBLE => PropertyValue::Standard(StandardProperty::Compatible(
                prop.value_as_string_list_iter()
                    .filter_map(|x| x.ok())
                    .map(|x| x.to_string())
                    .collect(),
            )),
            MODEL => PropertyValue::Standard(StandardProperty::Model(
                prop.value_as_string().unwrap().to_string(),
            )),
            PHANDLE => PropertyValue::Standard(StandardProperty::PHandle(PHandle(
                prop.value_as_u32().unwrap(),
            ))),
            STATUS => PropertyValue::Standard(StandardProperty::Status(
                prop.value_as_string().unwrap().try_into().unwrap(),
            )),
            ADDRESS_CELLS => PropertyValue::Standard(StandardProperty::AddressCells(
                AddressCellsValue(prop.value_as_u32().unwrap()),
            )),
            SIZE_CELLS => PropertyValue::Standard(StandardProperty::SizeCells(SizeCellsValue(
                prop.value_as_u32().unwrap(),
            ))),
            REG => PropertyValue::Standard(StandardProperty::Reg({
                let parent_addr_cells = node.parent().unwrap_or(node).address_cells().0 as usize;
                let parent_size_cells = node.parent().unwrap_or(node).size_cells().0 as usize;

                prop.value()
                    .to_vec()
                    .chunks_exact((parent_addr_cells + parent_size_cells) * 4)
                    .map(|chunk| {
                        let (addr_part, len_part) = chunk.split_at(parent_addr_cells * 4);

                        let addr_vec = addr_part
                            .chunks_exact(4)
                            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                            .collect();
                        let len_vec = len_part
                            .chunks_exact(4)
                            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                            .collect();

                        RegItemValue::new(addr_vec, len_vec)
                    })
                    .collect::<Vec<_>>()
            })),
            VIRTUAL_REG => {
                PropertyValue::Standard(StandardProperty::VirtualReg(prop.value_as_u32().unwrap()))
            }
            RANGES => PropertyValue::Standard(StandardProperty::Ranges({
                let addr_cells = node.address_cells().0 as usize;
                let parent_addr_cells = node.parent().unwrap_or(node).address_cells().0 as usize;
                let size_cells = node.size_cells().0 as usize;

                prop.value()
                    .to_vec()
                    .chunks_exact((addr_cells + parent_addr_cells + size_cells) * 4)
                    .map(|chunk| {
                        let (addr_part, rest) = chunk.split_at(addr_cells * 4);
                        let (parent_addr_part, len_part) = rest.split_at(parent_addr_cells * 4);

                        let addr_vec = addr_part
                            .chunks_exact(4)
                            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                            .collect();
                        let parent_addr_vec = parent_addr_part
                            .chunks_exact(4)
                            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                            .collect();
                        let len_vec = len_part
                            .chunks_exact(4)
                            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                            .collect();

                        RangesItemValue::new(addr_vec, parent_addr_vec, len_vec)
                    })
                    .collect::<Vec<_>>()
            })),
            DMA_RANGES => PropertyValue::Standard(StandardProperty::DmaRanges({
                let addr_cells = node.address_cells().0 as usize;
                let parent_addr_cells = node.parent().unwrap_or(node).address_cells().0 as usize;
                let size_cells = node.size_cells().0 as usize;

                prop.value()
                    .to_vec()
                    .chunks_exact((addr_cells + parent_addr_cells + size_cells) * 4)
                    .map(|chunk| {
                        let (addr_part, rest) = chunk.split_at(addr_cells * 4);
                        let (parent_addr_part, len_part) = rest.split_at(parent_addr_cells * 4);

                        let addr_vec = addr_part
                            .chunks_exact(4)
                            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                            .collect();
                        let parent_addr_vec = parent_addr_part
                            .chunks_exact(4)
                            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                            .collect();
                        let len_vec = len_part
                            .chunks_exact(4)
                            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                            .collect();

                        DmaRangesItemValue::new(addr_vec, parent_addr_vec, len_vec)
                    })
                    .collect::<Vec<_>>()
            })),
            DMA_COHERENT => PropertyValue::Standard(StandardProperty::DmaCoherent),
            DMA_NONCOHERENT => PropertyValue::Standard(StandardProperty::DmaNoncoherent),
            // Interrupt Properties
            INTERRUPTS => {
                PropertyValue::Interrupts(InterruptsProperty::Interrupts(Interrupts::from_raw(
                    prop.value()
                        .chunks_exact(4)
                        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                        .collect(),
                )))
            }
            INTERRUPTS_EXTENDED => PropertyValue::Interrupts(
                InterruptsProperty::ExtendedInterrupts(ExtendedInterrupts::from_raw(
                    prop.value()
                        .chunks_exact(4)
                        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                        .collect(),
                )),
            ),
            INTERRUPT_PARENT => PropertyValue::Interrupts(InterruptsProperty::InterruptParent(
                PHandle(prop.value_as_phandle().unwrap()),
            )),
            INTERRUPT_CELLS => PropertyValue::Interrupts(InterruptsProperty::InterruptCells(
                prop.value_as_u32().unwrap(),
            )),
            INTERRUPT_CONTROLLER => {
                PropertyValue::Interrupts(InterruptsProperty::InterruptController)
            }
            INTERRUPT_MAP => {
                PropertyValue::Interrupts(InterruptsProperty::InterruptMap(InterruptMap::from_raw(
                    prop.value()
                        .chunks_exact(4)
                        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                        .collect(),
                )))
            }
            INTERRUPT_MAP_MASK => {
                PropertyValue::Interrupts(InterruptsProperty::InterruptMapMask(InterruptMapMask(
                    prop.value()
                        .chunks_exact(4)
                        .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                        .collect(),
                )))
            }
            // Miscellaneous Properties
            CLOCK_FREQUENCY => PropertyValue::Miscellaneous(MiscellaneousProperty::ClockFrequency(
                match prop.value().len() {
                    4 => ClockFrequency::U32(prop.value_as_u32().unwrap()),
                    8 => ClockFrequency::U64(prop.value_as_u64().unwrap()),
                    _ => unreachable!(),
                },
            )),
            // Fallbacks
            _ if prop.value().is_empty() => PropertyValue::Empty,
            _ => PropertyValue::Unknown(UnknownProperty(prop.value().to_vec())),
        };

        Self {
            name: prop.name().to_string(),
            value,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &PropertyValue {
        &self.value
    }
}

#[derive(Debug)]
pub enum PropertyValue {
    Standard(StandardProperty),
    Interrupts(InterruptsProperty),
    Miscellaneous(MiscellaneousProperty),
    Unknown(UnknownProperty),
    Empty,
}

#[derive(Debug)]
pub struct UnknownProperty(Vec<u8>);

impl UnknownProperty {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }
}

impl TryInto<u32> for &UnknownProperty {
    type Error = ();

    fn try_into(self) -> Result<u32, Self::Error> {
        self.0
            .as_slice()
            .try_into()
            .map(u32::from_be_bytes)
            .map_err(|_| ())
    }
}

impl TryInto<PHandle> for &UnknownProperty {
    type Error = ();

    fn try_into(self) -> Result<PHandle, Self::Error> {
        self.try_into().map(|phandle| PHandle(phandle))
    }
}

impl TryInto<u64> for &UnknownProperty {
    type Error = ();

    fn try_into(self) -> Result<u64, Self::Error> {
        self.0
            .as_slice()
            .try_into()
            .map(u64::from_be_bytes)
            .map_err(|_| ())
    }
}

impl TryInto<String> for &UnknownProperty {
    type Error = ();

    fn try_into(self) -> Result<String, Self::Error> {
        CStr::from_bytes_with_nul(&self.0)
            .map_err(|_| ())
            .and_then(|cstr| cstr.to_str().map_err(|_| ()))
            .map(|cstr| cstr.to_string())
    }
}

impl TryInto<Vec<String>> for &UnknownProperty {
    type Error = ();

    fn try_into(self) -> Result<Vec<String>, Self::Error> {
        Ok(RawProp::new("", &self.0)
            .value_as_string_list_iter()
            .filter_map(|x| x.ok())
            .map(|x| x.to_string())
            .collect())
    }
}

impl<const N: usize> TryInto<Vec<[u32; N]>> for &UnknownProperty {
    type Error = ();

    fn try_into(self) -> Result<Vec<[u32; N]>, Self::Error> {
        todo!()
    }
}

impl<const N: usize, const M: usize> TryInto<Vec<([u32; N], [u32; M])>> for &UnknownProperty {
    type Error = ();

    fn try_into(self) -> Result<Vec<([u32; N], [u32; M])>, Self::Error> {
        todo!()
    }
}
