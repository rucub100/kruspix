
/// Representation of the Flattened Device Tree (FDT) header.
///
/// All fields are in big-endian format as per the DTSpec 0.4.
#[repr(C, align(8))]
pub struct FdtHeader {
    magic: u32,
    total_size: u32,
    off_dt_struct: u32,
    off_dt_strings: u32,
    off_mem_rsvmap: u32,
    version: u32,
    last_comp_version: u32,
    boot_cpuid_phys: u32,
    size_dt_strings: u32,
    size_dt_struct: u32,
}

impl FdtHeader {
    pub fn from_addr(addr: usize) -> Self {
        let ptr = addr as *const FdtHeader;
        unsafe { ptr.read() }
    }

    pub fn is_valid(&self) -> bool {
        u32::from_be(self.magic) == 0xd00dfeed
    }

    pub fn total_size(&self) -> u32 {
        u32::from_be(self.total_size)
    }

    pub fn version(&self) -> u32 {
        u32::from_be(self.version)
    }

    pub fn last_comp_version(&self) -> u32 {
        u32::from_be(self.last_comp_version)
    }

    pub fn structure_block_offset(&self) -> u32 {
        u32::from_be(self.off_dt_struct)
    }

    pub fn structure_block_size(&self) -> u32 {
        u32::from_be(self.size_dt_struct)
    }

    pub fn strings_block_offset(&self) -> u32 {
        u32::from_be(self.off_dt_strings)
    }

    pub fn strings_block_size(&self) -> u32 {
        u32::from_be(self.size_dt_strings)
    }

    pub fn mem_rsv_map_offset(&self) -> u32 {
        u32::from_be(self.off_mem_rsvmap)
    }

    pub fn boot_cpuid_phys(&self) -> u32 {
        u32::from_be(self.boot_cpuid_phys)
    }
}