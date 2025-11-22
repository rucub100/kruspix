//! Memory layout constants for the kernel.
//!
//! This module defines the virtual memory layout offsets used by the kernel.
//! Each region is 15TiB in size.


/// User space region.
pub const USER_MAP_OFFSET: usize = 0x0000_0000_0000_0000;

/// Unmapped region.
pub const UNUSED_MAP_OFFSET: usize = 0xffff_0000_0000_0000;

/// Linear mapping region (identity mapping of physical memory).
pub const LINEAR_MAP_OFFSET: usize = 0xffff_8000_0000_0000;

/// Heap region.
///
/// This region is used for dynamic memory allocation of the kernel heap.
pub const HEAP_MAP_OFFSET: usize = 0xffff_a000_0000_0000;

/// I/O peripherals region.
///
/// This region is reserved for memory-mapped I/O peripherals.
pub const IO_PERIPHERALS_MAP_OFFSET: usize = 0xffff_d000_0000_0000;
