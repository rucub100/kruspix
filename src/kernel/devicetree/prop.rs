use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::PHandle;
use super::fdt::raw_prop::RawProp;
use super::node::Node;
use super::std_prop::{
    ADDRESS_CELLS, AddressCellsValue, COMPATIBLE, DMA_COHERENT, DMA_NONCOHERENT, DMA_RANGES,
    DmaRangesItemValue, MODEL, PHANDLE, RANGES, REG, RangesItemValue, RegItemValue, SIZE_CELLS,
    STATUS, SizeCellsValue, StandardProperties, StandardProperty, VIRTUAL_REG,
};

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
            // TODO: match interrupt properties and other known properties
            _ => PropertyValue::Unknown(prop.value().to_vec()),
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
    Unknown(Vec<u8>),
}
