// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;

use crate::kernel::sync::OnceLock;

pub mod bcm2708_fb;
pub mod edid;

/// Trait for a memory-mapped framebuffer device.
pub trait FrameBufferDevice: Send + Sync {
    /// Returns the physical horizontal resolution in pixels.
    fn width(&self) -> u32;

    /// Returns the physical vertical resolution in pixels.
    fn height(&self) -> u32;

    /// Returns the row pitch (the scanline size) in bytes.
    /// Crucial for calculating offset mappings when width != stride.
    fn pitch(&self) -> u32;

    /// Returns the bits-per-pixel depth (e.g., 16, 24, 32).
    fn depth_bpp(&self) -> u32;

    /// Fills the entire framebuffer with a given 32-bit ARGB color value.
    fn fill(&self, color: u32);

    /// Writes a single pixel at coordinate `(x, y)` with the given 32-bit ARGB color value.
    /// Out-of-bounds coordinate parameters must be silently ignored.
    fn write_pixel(&self, x: u32, y: u32, color: u32);

    /// Returns a raw pointer to the start of the framebuffer's virtual address range.
    ///
    /// # Safety
    ///
    /// The pointer is valid only while the framebuffer remains mapped in the kernel page tables. 
    /// Callers must ensure all accesses stay within `height * pitch` bytes and use volatile semantics.
    fn buffer_ptr(&self) -> *mut u8;
}

static SYSTEM_FRAMEBUFFER: OnceLock<Arc<dyn FrameBufferDevice>> = OnceLock::new();

pub fn get_framebuffer() -> Option<Arc<dyn FrameBufferDevice>> {
    SYSTEM_FRAMEBUFFER.get().cloned()
}

pub fn register_framebuffer(fb: Arc<dyn FrameBufferDevice>) -> Result<(), ()> {
    SYSTEM_FRAMEBUFFER.set(fb).map_err(|_| ())
}
