// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use super::fdt_structure_block::{StructureBlockEntryKind, StructureBlockIter};
use crate::kernel::devicetree::node::{ALIASES, CHOSEN, CPUS, MEMORY, RESERVED_MEMORY, ROOT};

#[derive(Clone, Copy)]
pub struct RawNode<'a> {
    name: &'a str,
    props_ptr: *const u32,
    children_ptr: *const u32,
}

impl<'a> RawNode<'a> {
    pub fn new(name: &'a str, props_ptr: *const u32, children_ptr: *const u32) -> Self {
        Self {
            name,
            props_ptr,
            children_ptr,
        }
    }

    pub fn name(&self) -> &'a str {
        self.name
    }

    pub fn node_name(&self) -> &str {
        &self.name.split('@').next().unwrap_or(&self.name)
    }

    pub fn unit_address(&self) -> Option<&str> {
        self.name.split('@').skip(1).next()
    }

    pub fn is_root(&self) -> bool {
        self.node_name() == ROOT
    }

    pub fn is_aliases(&self) -> bool {
        self.node_name() == ALIASES
    }

    pub fn is_memory(&self) -> bool {
        self.node_name() == MEMORY
    }
    pub fn is_reserved_memory(&self) -> bool {
        self.node_name() == RESERVED_MEMORY
    }

    pub fn is_chosen(&self) -> bool {
        self.node_name() == CHOSEN
    }

    pub fn is_cpus(&self) -> bool {
        self.node_name() == CPUS
    }

    pub fn props_ptr(&self) -> *const u32 {
        self.props_ptr
    }

    pub fn children_ptr(&self) -> *const u32 {
        self.children_ptr
    }
}

pub struct NodeIter<'a> {
    structure_block_iter: StructureBlockIter<'a>,
    current_depth: isize,
    max_depth: isize,
}

impl<'a> NodeIter<'a> {
    pub fn new(token_be_ptr: *const u32, strings_block_address: usize, max_depth: isize) -> Self {
        assert!(max_depth > 0);

        NodeIter {
            structure_block_iter: StructureBlockIter::new_without_props(
                token_be_ptr,
                strings_block_address,
            ),
            current_depth: 0,
            max_depth,
        }
    }
}

impl<'a> Iterator for NodeIter<'a> {
    type Item = RawNode<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current_depth < 0 {
                return None;
            }

            let item = self.structure_block_iter.next();
            if item.is_none() {
                return None;
            }

            let result = item.unwrap();
            if result.is_err() {
                return None;
            }

            let entry = result.unwrap();

            match entry.into_kind() {
                StructureBlockEntryKind::BeginNode(node) => {
                    self.current_depth += 1;
                    if self.current_depth > self.max_depth {
                        continue;
                    }

                    return Some(node);
                }
                StructureBlockEntryKind::EndNode => {
                    self.current_depth -= 1;
                    continue;
                }
                _ => unreachable!(),
            }
        }
    }
}
