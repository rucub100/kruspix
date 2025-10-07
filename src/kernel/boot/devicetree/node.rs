use core::cmp::PartialEq;
use core::ffi::CStr;

use super::fdt_structure_block::{StructureBlockEntryKind, StructureBlockIter};

pub struct Node {
    name: &'static CStr,
    kind: NodeKind,
    // parent_ptr: *const u32,
    props_ptr: *const u32,
    // children_ptr: *const u32,
}

impl Node {
    fn new(name: &'static CStr, props_ptr: *const u32) -> Self {
        let kind = Self::_parse_kind(name);

        Node {
            name,
            kind,
            props_ptr,
        }
    }

    pub fn name(&self) -> &'static CStr {
        self.name
    }

    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    pub fn is_root(&self) -> bool {
        self.kind == NodeKind::Root
    }

    pub fn is_aliases(&self) -> bool {
        self.kind == NodeKind::Aliases
    }

    pub fn is_memory(&self) -> bool {
        self.kind == NodeKind::Memory
    }
    pub fn is_reserved_memory(&self) -> bool {
        self.kind == NodeKind::ReservedMemory
    }

    pub fn is_chosen(&self) -> bool {
        self.kind == NodeKind::Chosen
    }

    pub fn is_cpus(&self) -> bool {
        self.kind == NodeKind::Cpus
    }

    pub fn is_generic(&self) -> bool {
        self.kind == NodeKind::Generic
    }

    pub fn props_ptr(&self) -> *const u32 {
        self.props_ptr
    }

    fn _parse_kind(name: &'static CStr) -> NodeKind {
        match name.to_bytes() {
            b"" => NodeKind::Root,
            b"aliases" => NodeKind::Aliases,
            mem if mem.starts_with(b"memory") => {
                if mem.len() == 6 || mem[6] == b'@' {
                    NodeKind::Memory
                } else {
                    NodeKind::Generic
                }
            }
            rsv_mem if rsv_mem.starts_with(b"reserved-memory") => {
                if rsv_mem.len() == 15 || rsv_mem[15] == b'@' {
                    NodeKind::ReservedMemory
                } else {
                    NodeKind::Generic
                }
            }
            b"chosen" => NodeKind::Chosen,
            b"cpus" => NodeKind::Cpus,
            _ => NodeKind::Generic,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NodeKind {
    Root,
    Aliases,
    Memory,
    ReservedMemory,
    Chosen,
    Cpus,
    Generic,
}

pub struct NodeIter {
    structure_block_iter: StructureBlockIter,
    depth: isize,
}

impl NodeIter {
    pub fn new(token_be_ptr: *const u32, strings_block_address: usize) -> Self {
        NodeIter {
            structure_block_iter: StructureBlockIter::new_without_props(
                token_be_ptr,
                strings_block_address,
            ),
            depth: 0,
        }
    }
}

impl Iterator for NodeIter {
    type Item = Node;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.depth < 0 {
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

            match entry.kind() {
                StructureBlockEntryKind::BeginNode { name, props_ptr } => {
                    self.depth += 1;
                    return Some(Node::new(name, *props_ptr));
                }
                StructureBlockEntryKind::EndNode => {
                    self.depth -= 1;
                    continue;
                }
                _ => unreachable!(),
            }
        }
    }
}
