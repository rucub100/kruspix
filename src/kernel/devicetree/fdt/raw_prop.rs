// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::ffi::{CStr, c_char};
use core::marker::PhantomData;
use core::slice;

use crate::kernel::devicetree::std_prop::{
    ADDRESS_CELLS, COMPATIBLE, DMA_COHERENT, DMA_NONCOHERENT, DMA_RANGES, MODEL, PHANDLE, RANGES,
    REG, SIZE_CELLS, STATUS, VIRTUAL_REG,
};

use super::fdt_prop::FdtProp;
use super::fdt_structure_block::{FDT_NOP, FDT_PROP};

pub struct RawProp<'a> {
    name: &'a str,
    value: &'a [u8],
}

impl<'a> RawProp<'a> {
    pub fn new(name: &'a str, value: &'a [u8]) -> Self {
        RawProp { name, value }
    }

    pub fn name(&self) -> &'a str {
        self.name
    }

    pub fn value(&self) -> &'a [u8] {
        self.value
    }

    pub fn value_as_u32(&self) -> Result<u32, ()> {
        let bytes_count = &self.value.len() * 8;
        if bytes_count != 32 {
            return Err(());
        }

        let value_ptr = self.value.as_ptr() as *const u32;
        unsafe { Ok(u32::from_be(*value_ptr)) }
    }

    pub fn value_as_u64(&self) -> Result<u64, ()> {
        let bytes_count = &self.value.len() * 8;
        if bytes_count != 64 {
            return Err(());
        }

        let value_ptr = self.value.as_ptr() as *const u64;
        unsafe { Ok(u64::from_be(*value_ptr)) }
    }

    pub fn value_as_string(&self) -> Result<&'a str, ()> {
        if self.value.is_empty() {
            return Err(());
        }

        CStr::from_bytes_with_nul(self.value)
            .map_err(|_| ())?
            .to_str()
            .map_err(|_| ())
    }

    pub fn value_as_phandle(&self) -> Result<u32, ()> {
        self.value_as_u32()
    }

    pub fn value_as_string_list_iter(&self) -> StringListIter<'_> {
        StringListIter {
            string_list: self.value,
            current_index: 0,
        }
    }

    pub fn value_as_prop_encoded_array_cells_iter(&self, size: u32) -> impl Iterator<Item = usize> {
        assert!(size > 0);

        let chunk_size = size as usize * 4;
        assert_eq!(self.value.len() % chunk_size, 0);

        self.value.chunks(chunk_size).map(move |item| {
            let mut result: usize = 0;
            for chunk in item.chunks(4) {
                result <<= 32;
                result += u32::from_be_bytes(chunk.try_into().unwrap()) as usize;
            }

            result
        })
    }

    /// # Safety
    /// This function will panic if `size_1` or `size_2` is zero or greater than 2.
    pub fn value_as_prop_encoded_array_cells_pair_iter(
        &self,
        size_1: u32,
        size_2: u32,
    ) -> impl Iterator<Item = (usize, usize)> {
        assert!(size_1 > 0);
        assert!(size_1 <= 2);
        assert!(size_2 > 0);
        assert!(size_2 <= 2);

        let chunk_size = (size_1 + size_2) as usize * 4;
        assert_eq!(self.value.len() % chunk_size, 0);

        let first_range = ..size_1 as usize * 4;
        let second_range = size_1 as usize * 4..;

        self.value.chunks(chunk_size).map(move |chunk| {
            let mut first: usize = 0;
            let first_range = first_range.clone();
            for first_chunk in chunk[first_range].chunks(4) {
                first <<= 32;
                first += u32::from_be_bytes(first_chunk.try_into().unwrap()) as usize;
            }

            let mut second: usize = 0;
            let second_range = second_range.clone();
            for second_chunk in chunk[second_range].chunks(4) {
                second <<= 32;
                second += u32::from_be_bytes(second_chunk.try_into().unwrap()) as usize;
            }

            (first, second)
        })
    }

    pub fn value_as_prop_encoded_array_cells_triplet_iter(
        &self,
        size_1: u32,
        size_2: u32,
        size_3: u32,
    ) -> impl Iterator<Item = (usize, usize, usize)> {
        assert!(size_1 > 0);
        assert!(size_2 > 0);
        assert!(size_3 > 0);

        let chunk_size = (size_1 + size_2 + size_3) as usize * 4;
        assert_eq!(self.value.len() % chunk_size, 0);

        let first_range = ..size_1 as usize * 4;
        let second_range = size_1 as usize * 4..(size_1 + size_2) as usize * 4;
        let third_range = (size_1 + size_2) as usize * 4..;

        self.value.chunks(chunk_size).map(move |chunk| {
            let mut first: usize = 0;
            let first_range = first_range.clone();
            for first_chunk in chunk[first_range].chunks(4) {
                first <<= 32;
                first += u32::from_be_bytes(first_chunk.try_into().unwrap()) as usize;
            }

            let mut second: usize = 0;
            let second_range = second_range.clone();
            for second_chunk in chunk[second_range].chunks(4) {
                second <<= 32;
                second += u32::from_be_bytes(second_chunk.try_into().unwrap()) as usize;
            }

            let mut third: usize = 0;
            let third_range = third_range.clone();
            for third_chunk in chunk[third_range].chunks(4) {
                third <<= 32;
                third += u32::from_be_bytes(third_chunk.try_into().unwrap()) as usize;
            }

            (first, second, third)
        })
    }
}

pub struct StringListIter<'a> {
    string_list: &'a [u8],
    current_index: usize,
}

impl<'a> Iterator for StringListIter<'a> {
    type Item = Result<&'a str, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.string_list.len() {
            return None;
        }

        let slice = &self.string_list[self.current_index..];
        let cstr = CStr::from_bytes_until_nul(slice).map_err(|_| ());

        if cstr.is_ok() {
            let cstr_len = cstr.unwrap().to_bytes_with_nul().len();
            self.current_index += cstr_len;
        } else {
            self.current_index = self.string_list.len();
        }

        Some(cstr.and_then(|cstr| cstr.to_str().map_err(|_| ())))
    }
}

pub struct PropIter<'a> {
    prop_token_ptr: *const u32,
    strings_block_addr: usize,
    _marker: PhantomData<&'a ()>,
}

impl<'a> PropIter<'a> {
    pub fn new(prop_ptr: *const u32, strings_block_address: usize) -> Self {
        PropIter {
            strings_block_addr: strings_block_address,
            prop_token_ptr: prop_ptr,
            _marker: PhantomData,
        }
    }

    fn next_prop_token(&mut self) {
        unsafe {
            loop {
                let token = u32::from_be(*self.prop_token_ptr);

                if token == FDT_PROP {
                    break;
                }

                if token == FDT_NOP {
                    self.prop_token_ptr = self.prop_token_ptr.add(1);
                    continue;
                }

                self.prop_token_ptr = core::ptr::null();
                break;
            }
        }
    }
}

impl<'a> Iterator for PropIter<'a> {
    type Item = RawProp<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_prop_token();

        if self.prop_token_ptr.is_null() {
            return None;
        }

        unsafe {
            self.prop_token_ptr = self.prop_token_ptr.add(1);

            let prop_ptr = self.prop_token_ptr as *const FdtProp;
            self.prop_token_ptr = self.prop_token_ptr.byte_add(size_of::<FdtProp>());

            let prop = &*prop_ptr;
            let prop_value_len = prop.value_len() as usize;
            let prop_value_len_aligned = prop_value_len.next_multiple_of(4);
            let prop_value_ptr = self.prop_token_ptr as *const u8;
            self.prop_token_ptr = self.prop_token_ptr.byte_add(prop_value_len_aligned);

            assert!(self.prop_token_ptr.is_aligned());

            let prop_name_addr = self.strings_block_addr + prop.name_offset() as usize;
            let prop_name_ptr = prop_name_addr as *const c_char;
            let name = CStr::from_ptr(prop_name_ptr).to_str().unwrap();
            let value = slice::from_raw_parts(prop_value_ptr, prop_value_len);

            Some(RawProp { name, value })
        }
    }
}

pub enum PropValue {
    Empty,
    U32,
    U64,
    String,
    PropEncodedArray,
    PHandle,
    StringList,
}

#[derive(PartialEq)]
pub enum StandardProperty {
    Compatible,
    Model,
    PHandle,
    Status,
    AddressCells,
    SizeCells,
    Reg,
    VirtualReg,
    Ranges,
    DmaRanges,
    DmaCoherent,
    DmaNoncoherent,
}

impl TryFrom<&str> for StandardProperty {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            COMPATIBLE => Ok(StandardProperty::Compatible),
            MODEL => Ok(StandardProperty::Model),
            PHANDLE => Ok(StandardProperty::PHandle),
            STATUS => Ok(StandardProperty::Status),
            ADDRESS_CELLS => Ok(StandardProperty::AddressCells),
            SIZE_CELLS => Ok(StandardProperty::SizeCells),
            REG => Ok(StandardProperty::Reg),
            VIRTUAL_REG => Ok(StandardProperty::VirtualReg),
            RANGES => Ok(StandardProperty::Ranges),
            DMA_RANGES => Ok(StandardProperty::DmaRanges),
            DMA_COHERENT => Ok(StandardProperty::DmaCoherent),
            DMA_NONCOHERENT => Ok(StandardProperty::DmaNoncoherent),
            _ => Err(()),
        }
    }
}

impl From<StandardProperty> for &str {
    fn from(value: StandardProperty) -> Self {
        match value {
            StandardProperty::Compatible => COMPATIBLE,
            StandardProperty::Model => MODEL,
            StandardProperty::PHandle => PHANDLE,
            StandardProperty::Status => STATUS,
            StandardProperty::AddressCells => ADDRESS_CELLS,
            StandardProperty::SizeCells => SIZE_CELLS,
            StandardProperty::Reg => REG,
            StandardProperty::VirtualReg => VIRTUAL_REG,
            StandardProperty::Ranges => RANGES,
            StandardProperty::DmaRanges => DMA_RANGES,
            StandardProperty::DmaCoherent => DMA_COHERENT,
            StandardProperty::DmaNoncoherent => DMA_NONCOHERENT,
        }
    }
}
