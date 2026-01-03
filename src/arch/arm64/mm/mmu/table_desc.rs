// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::ops::{Deref, DerefMut};

use crate::mm::phys_to_virt;

use super::page_table::PageTable;
use super::{ADDR_MASK, PAGE_SIZE, attr::TableDescriptorAttributes};

#[repr(transparent)]
pub struct TableDescriptor(u64);

impl TableDescriptor {
    pub const fn new(next_level_table_addr: usize) -> Self {
        assert!(next_level_table_addr.is_multiple_of(PAGE_SIZE));
        Self((next_level_table_addr as u64) | 0b11)
    }

    pub const unsafe fn next_level_table(&self) -> &mut PageTable {
        let phys_table_addr = self.0 & ADDR_MASK;
        let table_addr = phys_to_virt(phys_table_addr as usize);
        unsafe { &mut *(table_addr as *mut PageTable) }
    }
}

impl Deref for TableDescriptor {
    type Target = u64;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TableDescriptor {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TableDescriptorAttributes for TableDescriptor {}
