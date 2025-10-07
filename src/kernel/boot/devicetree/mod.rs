use fdt_header::{FdtHeader, FdtHeaderPtrExt};
use fdt_reserve_entry::FdtReserveEntry;
use fdt_reserve_entry::FdtReserveEntryIter;
use fdt_structure_block::StructureBlockIter;

use node::{Node, NodeIter};
use prop::PropIter;

mod fdt_header;
mod fdt_prop;
mod fdt_reserve_entry;
mod fdt_structure_block;

pub mod node;
pub mod prop;

pub struct Fdt {
    address: usize,
}

impl Fdt {
    pub fn new(address: usize) -> Result<Self, ()> {
        let header = FdtHeader::at_addr(address);
        if header.is_valid() {
            Ok(Fdt { address })
        } else {
            Err(())
        }
    }

    pub fn memory_reservation_block_iter(&self) -> FdtReserveEntryIter {
        let header = FdtHeader::at_addr(self.address);
        let offset = header.mem_rsv_map_offset() as usize;
        let address = self.address + offset;
        let fdt_reserve_entry_ptr = address as *const FdtReserveEntry;
        fdt_reserve_entry_ptr.into()
    }

    pub fn structure_block_iter(&self) -> StructureBlockIter {
        let header = FdtHeader::at_addr(self.address);
        let structure_block_offset = header.structure_block_offset() as usize;
        let strings_block_offset = header.strings_block_offset() as usize;
        let structure_block_address = self.address + structure_block_offset;
        let strings_block_address = self.address + strings_block_offset;
        let token_be_ptr = structure_block_address as *const u32;
        StructureBlockIter::new(token_be_ptr, strings_block_address)
    }

    pub fn node_iter(&self) -> NodeIter {
        let header = FdtHeader::at_addr(self.address);
        let structure_block_offset = header.structure_block_offset() as usize;
        let strings_block_offset = header.strings_block_offset() as usize;
        let structure_block_address = self.address + structure_block_offset;
        let strings_block_address = self.address + strings_block_offset;
        let token_be_ptr = structure_block_address as *const u32;
        NodeIter::new(token_be_ptr, strings_block_address)
    }

    pub fn root_node(&self) -> Result<Node, ()> {
        let root = self.node_iter().find(|node| node.is_root());
        root.ok_or(())
    }

    pub fn aliases_node(&self) -> Option<Node> {
        self.node_iter().find(|node| node.is_aliases())
    }

    pub fn memory_node_iter(&self) -> impl Iterator<Item = Node> {
        self.node_iter().filter(|node| node.is_memory())
    }

    pub fn reserved_memory_node(&self) -> Option<Node> {
        self.node_iter().find(|node| node.is_reserved_memory())
    }

    pub fn chosen_node(&self) -> Option<Node> {
        self.node_iter().find(|node| node.is_chosen())
    }

    pub fn cpus_node(&self) -> Result<Node, ()> {
        let root = self.node_iter().find(|node| node.is_cpus());
        root.ok_or(())
    }

    pub fn prop_iter(&self, node: &Node) -> PropIter {
        let header = FdtHeader::at_addr(self.address);
        let strings_block_offset = header.strings_block_offset() as usize;
        let strings_block_address = self.address + strings_block_offset;
        PropIter::new(node.props_ptr(), strings_block_address)
    }
}
