//! Memory layout constants for the kernel.
//!
//! This module defines the virtual memory layout offsets used by the kernel.
//! Each region is 15TiB in size.

pub const PAGE_SIZE: usize = 4096;

pub const PAGE_TABLE_ENTRIES: usize = const {
    assert!(size_of::<usize>() == 8);
    assert!(size_of::<u64>() == 8);
    PAGE_SIZE / size_of::<usize>()
};

pub const PAGE_SHIFT: usize = 12;
pub const LEVEL_3_SHIFT: usize = PAGE_SHIFT;
pub const LEVEL_2_SHIFT: usize = LEVEL_3_SHIFT + 9;
pub const LEVEL_1_SHIFT: usize = LEVEL_2_SHIFT + 9;
pub const LEVEL_0_SHIFT: usize = LEVEL_1_SHIFT + 9;

/// User space region.
pub const USER_MAP_OFFSET: usize = 0x0000_0000_0000_0000;

/// Unmapped region.
pub const UNUSED_MAP_OFFSET: usize = 0xffff_0000_0000_0000;

/// Linear mapping region (identity mapping of physical memory).
pub const LINEAR_MAP_OFFSET: usize = 0xffff_8000_0000_0000;
/// Size of the linear mapping region (`16TiB`).
pub const LINEAR_MAP_SIZE: usize = 0x0000_1000_0000_0000;

/// Heap region.
///
/// This region is used for dynamic memory allocation of the kernel heap.
pub const HEAP_MAP_OFFSET: usize = 0xffff_a000_0000_0000;

/// I/O peripherals region.
///
/// This region is reserved for memory-mapped I/O peripherals.
pub const IO_PERIPHERALS_MAP_OFFSET: usize = 0xffff_d000_0000_0000;
