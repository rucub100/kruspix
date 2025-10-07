use core::ffi::{CStr, c_char};
use core::slice;

use super::fdt_prop::FdtProp;
use super::fdt_structure_block::{FDT_NOP, FDT_PROP};

pub struct Prop {
    name: &'static CStr,
    value: &'static [u8],
}

impl Prop {
    pub fn name(&self) -> &'static CStr {
        self.name
    }

    pub fn value(&self) -> &'static [u8] {
        self.value
    }
}

pub struct PropIter {
    prop_token_ptr: *const u32,
    strings_block_addr: usize,
}

impl PropIter {
    pub fn new(prop_ptr: *const u32, strings_block_address: usize) -> Self {
        PropIter {
            strings_block_addr: strings_block_address,
            prop_token_ptr: prop_ptr,
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

impl Iterator for PropIter {
    type Item = Prop;

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
            let name = CStr::from_ptr(prop_name_ptr);
            let value = slice::from_raw_parts(prop_value_ptr, prop_value_len);

            Some(Prop { name, value })
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

pub enum StandardProp {
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

pub enum InterruptGenDevProp {
    Interrupts,
    InterruptParent,
    InterruptsExtended,
}

pub enum InterruptControllersProp {
    InterruptCells,
    InterruptController,
}

pub enum InterruptNexusProp {
    InterruptMap,
    InterruptMapMask,
    InterruptCells,
}
