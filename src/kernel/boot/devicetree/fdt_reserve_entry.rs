use core::ptr;

/// Representation of a reserved memory entry in a Flattened Device Tree (FDT).
#[repr(C, align(8))]
pub struct FdtReserveEntry {
    address: u64,
    size: u64,
}

pub struct FdtReserveEntryIter {
    current: *const FdtReserveEntry,
}

impl From<*const FdtReserveEntry> for FdtReserveEntryIter {
    fn from(fdt_reserve_entry_ptr: *const FdtReserveEntry) -> Self {
        FdtReserveEntryIter { current: fdt_reserve_entry_ptr }
    }
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

impl Iterator for FdtReserveEntryIter {
    type Item = &'static FdtReserveEntry;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let entry = &*self.current;

            if entry.address() == 0 && entry.size() == 0 {
                return None;
            }

            self.current = self.current.offset(1);

            Some(entry)
        }
    }
}
