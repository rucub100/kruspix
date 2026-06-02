// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! Software drawing primitives for a 32-bpp framebuffer.
//!
//! [`DrawingContext`] wraps an [`Arc<dyn FrameBufferDevice>`] and exposes three
//! bulk operations used by the framebuffer console:
//!
//! * [`fill_rect`] — solid-color rectangle fill (issues `memory_barrier` at end)
//! * [`copy_region`] — pixel-region copy for scroll-up (issues `memory_barrier` at end)
//! * [`draw_glyph`] — blit one 8-row glyph (NO `memory_barrier`; caller is responsible)
//!
//! All coordinate parameters that fall outside the physical framebuffer dimensions
//! are silently clamped; no bounds-check panics occur.

use alloc::sync::Arc;

use crate::arch::cpu::memory_barrier;

use super::FrameBufferDevice;
use super::font::{FONT_HEIGHT, FONT_WIDTH};

pub struct DrawingContext {
    fb: Arc<dyn FrameBufferDevice>,
}

impl DrawingContext {
    pub fn new(fb: Arc<dyn FrameBufferDevice>) -> Self {
        Self { fb }
    }

    /// Horizontal resolution of the underlying framebuffer in pixels.
    pub fn width(&self) -> u32 {
        self.fb.width()
    }

    /// Vertical resolution of the underlying framebuffer in pixels.
    pub fn height(&self) -> u32 {
        self.fb.height()
    }

    /// Scanline pitch of the underlying framebuffer in bytes.
    pub fn pitch(&self) -> u32 {
        self.fb.pitch()
    }

    /// DMA-visible bus address of the underlying framebuffer.
    pub fn bus_addr(&self) -> u32 {
        self.fb.bus_addr()
    }

    /// Fills a rectangle with `color` (0xAARRGGBB).
    ///
    /// The rectangle is clamped to the framebuffer boundary. Out-of-bounds
    /// input is silently ignored.
    ///
    /// Issues a `memory_barrier()` after all writes to ensure the GPU sees the
    /// updated pixels before the next cache-coherent bus transaction.
    pub fn fill_rect(&self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        let fb_w = self.fb.width();
        let fb_h = self.fb.height();
        if x >= fb_w || y >= fb_h || w == 0 || h == 0 {
            return;
        }
        let x_end = (x + w).min(fb_w);
        let y_end = (y + h).min(fb_h);
        let pitch_pixels = self.fb.pitch() / 4;

        // SAFETY: buffer_ptr() is valid for height*pitch bytes while the
        //   framebuffer remains mapped. The 32-bpp contract guarantees
        //   pitch = width_pixels * 4, so all (row, col) pairs computed below
        //   address memory within [buffer_ptr, buffer_ptr + height*pitch).
        //   write_volatile prevents the compiler from eliding or reordering writes.
        let base = self.fb.buffer_ptr() as *mut u32;

        for row in y..y_end {
            for col in x..x_end {
                let offset = row * pitch_pixels + col;
                unsafe {
                    core::ptr::write_volatile(base.add(offset as usize), color);
                }
            }
        }

        memory_barrier();
    }

    /// Copies a rectangular pixel region from `(src_x, src_y)` to `(dst_x, dst_y)`.
    ///
    /// Rows are iterated in **forward order** (top to bottom), which is correct
    /// for the scroll-up use case where `dst_y < src_y`.
    ///
    /// Dimensions are clamped so that neither source nor destination rectangle
    /// extends beyond the framebuffer boundary.
    ///
    /// Issues a `memory_barrier()` after all writes.
    pub fn copy_region(
        &self,
        src_x: u32,
        src_y: u32,
        dst_x: u32,
        dst_y: u32,
        w: u32,
        h: u32,
    ) {
        let fb_w = self.fb.width();
        let fb_h = self.fb.height();

        if src_x >= fb_w || src_y >= fb_h || dst_x >= fb_w || dst_y >= fb_h {
            return;
        }

        let w = w
            .min(fb_w.saturating_sub(src_x))
            .min(fb_w.saturating_sub(dst_x));
        let h = h
            .min(fb_h.saturating_sub(src_y))
            .min(fb_h.saturating_sub(dst_y));

        if w == 0 || h == 0 {
            return;
        }

        let pitch_pixels = self.fb.pitch() / 4;

        // SAFETY: Same invariants as fill_rect. Both src and dst offsets are
        //   bounded: (src_y + h - 1) < fb_h and (dst_y + h - 1) < fb_h after
        //   clamping above. read_volatile/write_volatile prevent UB from
        //   aliasing and ensure hardware-visible ordering within the loop.
        let base = self.fb.buffer_ptr() as *mut u32;

        for row in 0..h {
            for col in 0..w {
                let src_off = (src_y + row) * pitch_pixels + (src_x + col);
                let dst_off = (dst_y + row) * pitch_pixels + (dst_x + col);
                unsafe {
                    let pixel = core::ptr::read_volatile(base.add(src_off as usize));
                    core::ptr::write_volatile(base.add(dst_off as usize), pixel);
                }
            }
        }

        memory_barrier();
    }

    /// Blits one 8-wide glyph bitmap at pixel position `(x, y)`.
    ///
    /// `bitmap` must contain exactly [`FONT_HEIGHT`] bytes (one byte per
    /// scanline). Bit 7 of each byte is the left-most pixel; bit 0 is the
    /// right-most. A set bit renders `fg`; a clear bit renders `bg`.
    ///
    /// **Does not** issue a `memory_barrier()`. The caller must issue one after
    /// all glyphs for a single console write are blitted.
    pub fn draw_glyph(&self, x: u32, y: u32, bitmap: &[u8], fg: u32, bg: u32) {
        let fb_w = self.fb.width();
        let fb_h = self.fb.height();
        if x >= fb_w || y >= fb_h {
            return;
        }

        let pitch_pixels = self.fb.pitch() / 4;

        // SAFETY: py and px are checked against fb_h and fb_w on every
        //   iteration. The 32-bpp contract means offset = py*pitch_pixels + px
        //   stays within the mapped framebuffer region.
        let base = self.fb.buffer_ptr() as *mut u32;

        for (row, &byte) in bitmap.iter().enumerate().take(FONT_HEIGHT as usize) {
            let py = y + row as u32;
            if py >= fb_h {
                break;
            }
            for bit in 0u32..FONT_WIDTH {
                let px = x + bit;
                if px >= fb_w {
                    break;
                }
                let color = if byte & (0x80u8 >> bit) != 0 { fg } else { bg };
                let offset = py * pitch_pixels + px;
                unsafe {
                    core::ptr::write_volatile(base.add(offset as usize), color);
                }
            }
        }
        // No memory_barrier here — caller is responsible.
    }
}
