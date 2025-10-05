use fdt_header::{FdtHeader, FdtHeaderPtrExt};
use fdt_reserve_entry::FdtReserveEntry;
use fdt_reserve_entry::FdtReserveEntryIter;
use fdt_structure_block::StructureBlockIter;
use node::NodeIter;

pub mod fdt_header;
pub mod fdt_prop;
pub mod fdt_reserve_entry;
pub mod fdt_structure_block;
mod node;
mod prop;

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

    pub fn nodes_iter(&self) -> NodeIter {
        let header = FdtHeader::at_addr(self.address);
        let structure_block_offset = header.structure_block_offset() as usize;
        let strings_block_offset = header.strings_block_offset() as usize;
        let structure_block_address = self.address + structure_block_offset;
        let strings_block_address = self.address + strings_block_offset;
        let token_be_ptr = structure_block_address as *const u32;
        NodeIter::new(token_be_ptr, strings_block_address)
    }
}
