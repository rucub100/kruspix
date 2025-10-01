use crate::kernel::boot::devicetree::fdt_prop::FdtProp;
use core::ffi::{CStr, c_char};
use core::ptr;
use core::slice;

pub const FDT_BEGIN_NODE: u32 = const { 0x00000001 };
pub const FDT_END_NODE: u32 = const { 0x00000002 };
pub const FDT_PROP: u32 = const { 0x00000003 };
pub const FDT_NOP: u32 = const { 0x00000004 };
pub const FDT_END: u32 = const { 0x00000009 };

pub enum StructureBlockEntry {
    BeginNode(&'static CStr),
    EndNode,
    Prop {
        name: &'static CStr,
        value: &'static [u8],
    },
}

pub struct StructureBlockPtr {
    strings_block_addr: usize,
    token_be_ptr: *const u32,
    prev_token: u32,
}

impl StructureBlockPtr {
    pub fn new(token_be_ptr: *const u32, strings_block_address: usize) -> Self {
        StructureBlockPtr {
            strings_block_addr: strings_block_address,
            token_be_ptr,
            prev_token: 0,
        }
    }

    fn next_non_nop_token(&mut self) -> u32 {
        unsafe {
            loop {
                let token_be = ptr::read(self.token_be_ptr);
                let token = u32::from_be(token_be);

                if token != FDT_NOP {
                    return token;
                }

                self.token_be_ptr = self.token_be_ptr.add(1);
            }
        }
    }
}

impl Iterator for StructureBlockPtr {
    type Item = Result<StructureBlockEntry, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.prev_token == FDT_END || self.token_be_ptr.is_null() {
                return None;
            }

            let token = self.next_non_nop_token();

            match token {
                FDT_BEGIN_NODE => {
                    let name_ptr = self.token_be_ptr.add(1) as *const c_char;
                    let name = CStr::from_ptr(name_ptr);
                    let mut name_parts_count = 1;

                    loop {
                        let name_part_ptr = self.token_be_ptr.add(name_parts_count);
                        let name_part = ptr::read(name_part_ptr);
                        if name_part <= 0x00FFFFFF {
                            break;
                        }
                        name_parts_count += 1;

                        // node name must not exceed (8 * 4 - 1) = 31 chars
                        if name_parts_count > 8 {
                            return Some(Err(()));
                        }
                    }

                    self.token_be_ptr = self.token_be_ptr.add(1 + name_parts_count);
                    self.prev_token = token;
                    Some(Ok(StructureBlockEntry::BeginNode(name)))
                }
                FDT_END_NODE => {
                    self.token_be_ptr = self.token_be_ptr.add(1);
                    self.prev_token = token;
                    Some(Ok(StructureBlockEntry::EndNode))
                }
                FDT_PROP => {
                    if self.prev_token == FDT_END_NODE {
                        return Some(Err(()));
                    }

                    self.token_be_ptr = self.token_be_ptr.add(1);

                    let prop_ptr = self.token_be_ptr as *const FdtProp;
                    self.token_be_ptr = self.token_be_ptr.byte_add(size_of::<FdtProp>());

                    let prop = ptr::read(prop_ptr);
                    let prop_value_len = prop.value_len() as usize;
                    let prop_value_len_aligned = prop_value_len.next_multiple_of(4);
                    let prop_value_ptr = self.token_be_ptr as *const u8;
                    let prop_value = slice::from_raw_parts(prop_value_ptr, prop_value_len);
                    self.token_be_ptr = self.token_be_ptr.byte_add(prop_value_len_aligned);

                    assert!(self.token_be_ptr.is_aligned());

                    let prop_name_addr = self.strings_block_addr + prop.name_offset() as usize;
                    let prop_name_ptr = prop_name_addr as *const c_char;
                    let prop_name = CStr::from_ptr(prop_name_ptr);

                    self.prev_token = token;
                    Some(Ok(StructureBlockEntry::Prop {
                        name: prop_name,
                        value: prop_value,
                    }))
                }
                FDT_END => {
                    self.token_be_ptr = ptr::null();
                    let prev_token = self.prev_token;
                    self.prev_token = token;

                    if prev_token == FDT_BEGIN_NODE || prev_token == FDT_PROP {
                        return Some(Err(()));
                    }

                    None
                }
                _ => {
                    self.token_be_ptr = ptr::null();
                    self.prev_token = token;
                    Some(Err(()))
                }
            }
        }
    }
}
