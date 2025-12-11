use core::marker::PhantomData;

/// Representation of a reserved memory entry in a Flattened Device Tree (FDT).
#[repr(C, align(8))]
pub struct FdtReserveEntry {
    address: u64,
    size: u64,
}

pub struct FdtReserveEntryIter<'a> {
    current: *const FdtReserveEntry,
    _marker: PhantomData<&'a FdtReserveEntry>,
}

impl<'a> From<*const FdtReserveEntry> for FdtReserveEntryIter<'a> {
    fn from(fdt_reserve_entry_ptr: *const FdtReserveEntry) -> Self {
        FdtReserveEntryIter {
            current: fdt_reserve_entry_ptr,
            _marker: PhantomData,
        }
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

impl<'a> Iterator for FdtReserveEntryIter<'a> {
    type Item = &'a FdtReserveEntry;

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
