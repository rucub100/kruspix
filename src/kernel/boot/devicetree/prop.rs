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

    pub fn value_as_string(&self) -> Result<&'static CStr, ()> {
        if self.value.is_empty() {
            return Err(());
        }

        CStr::from_bytes_with_nul(self.value).map_err(|_| ())
    }

    pub fn value_as_phandle(&self) -> Result<u32, ()> {
        let bytes_count = &self.value.len() * 8;
        if bytes_count != 32 {
            return Err(());
        }

        let value_ptr = self.value.as_ptr() as *const u32;
        unsafe { Ok(*value_ptr) }
    }

    pub fn value_as_string_list_iter(&self) -> StringListIter {
        StringListIter {
            string_list: self.value,
            current_index: 0,
        }
    }
}

pub struct StringListIter {
    string_list: &'static [u8],
    current_index: usize,
}

impl Iterator for StringListIter {
    type Item = Result<&'static CStr, ()>;

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

        Some(cstr)
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

impl TryFrom<&[u8]> for StandardProp {
    type Error = ();

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value {
            b"compatible" => Ok(StandardProp::Compatible),
            b"model" => Ok(StandardProp::Model),
            b"phandle" => Ok(StandardProp::PHandle),
            b"status" => Ok(StandardProp::Status),
            b"#address-cells" => Ok(StandardProp::AddressCells),
            b"#size-cells" => Ok(StandardProp::SizeCells),
            b"reg" => Ok(StandardProp::Reg),
            b"virtual-reg" => Ok(StandardProp::VirtualReg),
            b"ranges" => Ok(StandardProp::Ranges),
            b"dma-ranges" => Ok(StandardProp::DmaRanges),
            b"dma-coherent" => Ok(StandardProp::DmaCoherent),
            b"dma-noncoherent" => Ok(StandardProp::DmaNoncoherent),
            _ => Err(()),
        }
    }
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

pub enum ChassisType {
    Desktop,
    Laptop,
    Convertible,
    Server,
    Tablet,
    Handset,
    Watch,
    Embedded,
}
