use fdt_header::{FdtHeader, FdtHeaderPtrExt};
use fdt_reserve_entry::FdtReserveEntryPtr;
use crate::kernel::boot::devicetree::fdt_reserve_entry::FdtReserveEntry;

pub mod fdt_header;
pub mod fdt_prop;
pub mod fdt_reserve_entry;
pub mod fdt_structure_block;

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

    pub fn memory_reservation_block(&self) -> FdtReserveEntryPtr {
        let header = FdtHeader::at_addr(self.address);
        let offset = header.mem_rsv_map_offset();
        let address = self.address + offset as usize;
        let fdt_reserve_entry_ptr = address as *const FdtReserveEntry;
        fdt_reserve_entry_ptr.into()
    }
}
