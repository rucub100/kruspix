/// Representation of a reserved memory entry in a Flattened Device Tree (FDT).
#[repr(C, align(8))]
pub struct FdtReserveEntry {
    address: u64,
    size: u64,
}

impl FdtReserveEntry {
    /// Returns the starting physical address of the reserved memory region.
    pub fn address(&self) -> u64 {
        u64::from_be(self.address)
    }

    /// Returns the size of the reserved memory region in bytes.
    pub fn size(&self) -> u64 {
        u64::from_be(self.size)
    }
}