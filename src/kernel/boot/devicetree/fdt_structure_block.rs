use core::ffi::{CStr, c_char};
use core::ptr;
use core::slice;

use super::fdt_prop::FdtProp;
use super::node::Node;

pub const FDT_BEGIN_NODE: u32 = const { 0x00000001 };
pub const FDT_END_NODE: u32 = const { 0x00000002 };
pub const FDT_PROP: u32 = const { 0x00000003 };
pub const FDT_NOP: u32 = const { 0x00000004 };
pub const FDT_END: u32 = const { 0x00000009 };

pub struct StructureBlockEntry {
    kind: StructureBlockEntryKind,
}

pub enum StructureBlockEntryKind {
    BeginNode {
        name: &'static CStr,
        props_ptr: *const u32,
    },
    EndNode,
    Prop {
        name: &'static CStr,
        value: &'static [u8],
    },
}

impl StructureBlockEntry {
    pub fn kind(&self) -> &StructureBlockEntryKind {
        &self.kind
    }
}

pub struct StructureBlockIter {
    strings_block_addr: usize,
    token_be_ptr: *const u32,
    prev_token: u32,
    skip_props: bool,
}

impl StructureBlockIter {
    pub fn new(token_be_ptr: *const u32, strings_block_address: usize) -> Self {
        StructureBlockIter {
            strings_block_addr: strings_block_address,
            token_be_ptr,
            prev_token: 0,
            skip_props: false,
        }
    }

    pub fn new_without_props(token_be_ptr: *const u32, strings_block_address: usize) -> Self {
        StructureBlockIter {
            strings_block_addr: strings_block_address,
            token_be_ptr,
            prev_token: 0,
            skip_props: true,
        }
    }

    fn next_non_nop_token(&mut self) -> u32 {
        unsafe {
            loop {
                let token = u32::from_be(*self.token_be_ptr);

                if token != FDT_NOP {
                    return token;
                }

                self.token_be_ptr = self.token_be_ptr.add(1);
            }
        }
    }

    fn next_non_nop_token_without_props(&mut self) -> u32 {
        unsafe {
            loop {
                let token = self.next_non_nop_token();

                if token != FDT_PROP {
                    return token;
                }

                self.token_be_ptr = self.token_be_ptr.add(1);

                let prop_ptr = self.token_be_ptr as *const FdtProp;
                self.token_be_ptr = self.token_be_ptr.byte_add(size_of::<FdtProp>());

                let prop = &*prop_ptr;
                let prop_value_len = prop.value_len() as usize;
                let prop_value_len_aligned = prop_value_len.next_multiple_of(4);
                self.token_be_ptr = self.token_be_ptr.byte_add(prop_value_len_aligned);

                assert!(self.token_be_ptr.is_aligned());
            }
        }
    }
}

impl Iterator for StructureBlockIter {
    type Item = Result<StructureBlockEntry, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.prev_token == FDT_END || self.token_be_ptr.is_null() {
                return None;
            }

            let token = if self.skip_props {
                self.next_non_nop_token_without_props()
            } else {
                self.next_non_nop_token()
            };

            match token {
                FDT_BEGIN_NODE => {
                    let name_ptr = self.token_be_ptr.add(1) as *const c_char;
                    let name = CStr::from_ptr(name_ptr);
                    let mut name_parts_count = 1;

                    loop {
                        let name_part_ptr = self.token_be_ptr.add(name_parts_count);
                        let name_part = *name_part_ptr;
                        if name_part <= 0x00FFFFFF {
                            break;
                        }
                        name_parts_count += 1;

                        // node name must not exceed (8 * 4 - 1) = 31 chars
                        // we also assume the @unit-address part to have (5 * 4 - 3) 17 chars max
                        // so we limit the total number of name parts to 13 (13 * 4 - 1) = 51 chars
                        if name_parts_count > 13 {
                            self.token_be_ptr = ptr::null();
                            self.prev_token = token;
                            return Some(Err(()));
                        }
                    }

                    let token_ptr = self.token_be_ptr;
                    self.token_be_ptr = self.token_be_ptr.add(1 + name_parts_count);
                    self.prev_token = token;

                    let mut props_ptr = ptr::null();
                    let next_token = self.next_non_nop_token();
                    if next_token == FDT_PROP {
                        props_ptr = self.token_be_ptr;
                    }

                    Some(Ok(StructureBlockEntry {
                        kind: StructureBlockEntryKind::BeginNode { name, props_ptr },
                    }))
                }
                FDT_END_NODE => {
                    let token_ptr = self.token_be_ptr;
                    self.token_be_ptr = self.token_be_ptr.add(1);
                    self.prev_token = token;
                    Some(Ok(StructureBlockEntry {
                        kind: StructureBlockEntryKind::EndNode,
                    }))
                }
                FDT_PROP => {
                    if self.prev_token == FDT_END_NODE {
                        self.token_be_ptr = ptr::null();
                        self.prev_token = token;
                        return Some(Err(()));
                    }

                    let token_ptr = self.token_be_ptr;
                    self.token_be_ptr = self.token_be_ptr.add(1);

                    let prop_ptr = self.token_be_ptr as *const FdtProp;
                    self.token_be_ptr = self.token_be_ptr.byte_add(size_of::<FdtProp>());

                    let prop = &*prop_ptr;
                    let prop_value_len = prop.value_len() as usize;
                    let prop_value_len_aligned = prop_value_len.next_multiple_of(4);
                    let prop_value_ptr = self.token_be_ptr as *const u8;
                    self.token_be_ptr = self.token_be_ptr.byte_add(prop_value_len_aligned);

                    assert!(self.token_be_ptr.is_aligned());

                    let prop_name_addr = self.strings_block_addr + prop.name_offset() as usize;
                    let prop_name_ptr = prop_name_addr as *const c_char;
                    let prop_name = CStr::from_ptr(prop_name_ptr);
                    let prop_value = slice::from_raw_parts(prop_value_ptr, prop_value_len);

                    self.prev_token = token;
                    Some(Ok(StructureBlockEntry {
                        kind: StructureBlockEntryKind::Prop {
                            name: prop_name,
                            value: prop_value,
                        },
                    }))
                }
                FDT_END => {
                    self.token_be_ptr = ptr::null();
                    let prev_token = self.prev_token;
                    self.prev_token = token;

                    if prev_token == FDT_BEGIN_NODE || prev_token == FDT_PROP {
                        self.token_be_ptr = ptr::null();
                        self.prev_token = token;
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
