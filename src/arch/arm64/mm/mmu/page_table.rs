use core::slice::IterMut;

use crate::mm::{alloc_page, virt_to_phys};

use super::PAGE_TABLE_ENTRIES;

// FIXME: for now the align is hardcoded to 4096, but it should be derived form the page size
#[repr(C, align(4096))]
pub struct PageTable {
    descriptors: [u64; PAGE_TABLE_ENTRIES],
}

impl PageTable {
    pub fn new() -> &'static mut Self {
        unsafe {
            let table_ptr = alloc_page() as *mut PageTable;
            let table = &mut *table_ptr;
            table.descriptors.iter_mut().for_each(|d| *d = 0);

            table
        }
    }

    pub fn phys_addr(&self) -> usize {
        virt_to_phys(self as *const _ as usize)
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, u64> {
        self.descriptors.iter_mut()
    }

    pub fn get_descriptor(&self, index: usize) -> u64 {
        self.descriptors[index]
    }

    pub fn set_descriptor(&mut self, index: usize, desc: u64) {
        self.descriptors[index] = desc;
    }
}
