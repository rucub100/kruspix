// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::ptr;

const FDT_MAGIC: u32 = 0xd00dfeed;

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

pub trait FdtHeaderPtrExt {
    fn is_valid(&self) -> bool;
    fn total_size(&self) -> u32;
    fn version(&self) -> u32;
    fn last_comp_version(&self) -> u32;
    fn structure_block_offset(&self) -> u32;
    fn structure_block_size(&self) -> u32;
    fn strings_block_offset(&self) -> u32;
    fn strings_block_size(&self) -> u32;
    fn mem_rsv_map_offset(&self) -> u32;
    fn boot_cpuid_phys(&self) -> u32;
}

impl FdtHeader {
    pub fn at_addr(addr: usize) -> *const Self {
        addr as *const FdtHeader
    }
}

impl FdtHeaderPtrExt for *const FdtHeader {
    fn is_valid(&self) -> bool {
        unsafe {
            /* we cannot deref magic because it's the very first field and might be at
            address 0 which is invalid to deref (null pointer) */
            let magic_ptr = ptr::addr_of!((**self).magic);
            let magic_be = ptr::read(magic_ptr);
            u32::from_be(magic_be) == FDT_MAGIC
        }
    }

    fn total_size(&self) -> u32 {
        unsafe {
            let total_size_ptr = ptr::addr_of!((**self).total_size);
            u32::from_be(*total_size_ptr)
        }
    }

    fn version(&self) -> u32 {
        unsafe {
            let version_ptr = ptr::addr_of!((**self).version);
            u32::from_be(*version_ptr)
        }
    }

    fn last_comp_version(&self) -> u32 {
        unsafe {
            let last_comp_version_ptr = ptr::addr_of!((**self).last_comp_version);
            u32::from_be(*last_comp_version_ptr)
        }
    }

    fn structure_block_offset(&self) -> u32 {
        unsafe {
            let off_dt_struct_ptr = ptr::addr_of!((**self).off_dt_struct);
            u32::from_be(*off_dt_struct_ptr)
        }
    }

    fn structure_block_size(&self) -> u32 {
        unsafe {
            let size_dt_struct_ptr = ptr::addr_of!((**self).size_dt_struct);
            u32::from_be(*size_dt_struct_ptr)
        }
    }

    fn strings_block_offset(&self) -> u32 {
        unsafe {
            let off_dt_strings_ptr = ptr::addr_of!((**self).off_dt_strings);
            u32::from_be(*off_dt_strings_ptr)
        }
    }

    fn strings_block_size(&self) -> u32 {
        unsafe {
            let size_dt_strings_ptr = ptr::addr_of!((**self).size_dt_strings);
            u32::from_be(*size_dt_strings_ptr)
        }
    }

    fn mem_rsv_map_offset(&self) -> u32 {
        unsafe {
            let off_mem_rsvmap_ptr = ptr::addr_of!((**self).off_mem_rsvmap);
            u32::from_be(*off_mem_rsvmap_ptr)
        }
    }

    fn boot_cpuid_phys(&self) -> u32 {
        unsafe {
            let boot_cpuid_phys_ptr = ptr::addr_of!((**self).boot_cpuid_phys);
            u32::from_be(*boot_cpuid_phys_ptr)
        }
    }
}
