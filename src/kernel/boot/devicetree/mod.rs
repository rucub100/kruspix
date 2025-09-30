use fdt_header::{FdtHeader, FdtHeaderPtrExt};

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
}
