// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::ffi::{CStr, c_char};
use core::marker::PhantomData;
use core::ptr;
use core::slice;

use super::fdt_prop::FdtProp;
use super::raw_node::RawNode;
use super::raw_prop::RawProp;

pub const FDT_BEGIN_NODE: u32 = const { 0x00000001 };
pub const FDT_END_NODE: u32 = const { 0x00000002 };
pub const FDT_PROP: u32 = const { 0x00000003 };
pub const FDT_NOP: u32 = const { 0x00000004 };
pub const FDT_END: u32 = const { 0x00000009 };

pub struct StructureBlockEntry<'a> {
    kind: StructureBlockEntryKind<'a>,
}

pub enum StructureBlockEntryKind<'a> {
    BeginNode(RawNode<'a>),
    EndNode,
    Prop(RawProp<'a>),
}

impl<'a> StructureBlockEntry<'a> {
    pub fn kind(&self) -> &'a StructureBlockEntryKind<'_> {
        &self.kind
    }

    pub fn into_kind(self) -> StructureBlockEntryKind<'a> {
        self.kind
    }
}

pub struct StructureBlockIter<'a> {
    strings_block_addr: usize,
    token_be_ptr: *const u32,
    prev_token: u32,
    skip_props: bool,
    _marker: PhantomData<&'a ()>,
}

impl<'a> StructureBlockIter<'a> {
    pub fn new(token_be_ptr: *const u32, strings_block_address: usize) -> Self {
        StructureBlockIter {
            strings_block_addr: strings_block_address,
            token_be_ptr,
            prev_token: 0,
            skip_props: false,
            _marker: PhantomData,
        }
    }

    pub fn new_without_props(token_be_ptr: *const u32, strings_block_address: usize) -> Self {
        StructureBlockIter {
            strings_block_addr: strings_block_address,
            token_be_ptr,
            prev_token: 0,
            skip_props: true,
            _marker: PhantomData,
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

    fn peek_next_non_nop_token_without_props(&mut self) -> (u32, *const u32) {
        let saved_token_be_ptr = self.token_be_ptr;
        let token = self.next_non_nop_token_without_props();
        let ptr = self.token_be_ptr;
        self.token_be_ptr = saved_token_be_ptr;
        (token, ptr)
    }
}

impl<'a> Iterator for StructureBlockIter<'a> {
    type Item = Result<StructureBlockEntry<'a>, ()>;

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
                    let name = CStr::from_ptr(name_ptr).to_str().ok()?;
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

                    self.token_be_ptr = self.token_be_ptr.add(1 + name_parts_count);
                    self.prev_token = token;

                    let mut props_ptr = ptr::null();
                    let mut children_ptr = ptr::null();
                    let next_token = self.next_non_nop_token();
                    if next_token == FDT_PROP {
                        props_ptr = self.token_be_ptr;
                    } else if next_token == FDT_BEGIN_NODE {
                        children_ptr = self.token_be_ptr;
                    }
                    let (next_without_props, ptr) = self.peek_next_non_nop_token_without_props();
                    children_ptr = if next_without_props == FDT_BEGIN_NODE {
                        ptr
                    } else {
                        ptr::null()
                    };

                    Some(Ok(StructureBlockEntry {
                        kind: StructureBlockEntryKind::BeginNode(RawNode::new(
                            name,
                            props_ptr,
                            children_ptr,
                        )),
                    }))
                }
                FDT_END_NODE => {
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
                    let prop_name = CStr::from_ptr(prop_name_ptr).to_str().unwrap();
                    let prop_value = slice::from_raw_parts(prop_value_ptr, prop_value_len);

                    self.prev_token = token;
                    Some(Ok(StructureBlockEntry {
                        kind: StructureBlockEntryKind::Prop(RawProp::new(prop_name, prop_value)),
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
