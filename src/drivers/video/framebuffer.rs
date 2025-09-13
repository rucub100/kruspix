// src/framebuffer.rs

use crate::bcm2835_mailbox::{mailbox_call, MessageBuffer};
use core::ptr::write_volatile;

// A simple 8x16 font embedded in the code.
// This is just a placeholder; you'd need the full font data.
// https://github.com/ercanersoy/PSF-Fonts
// GNU GPL v2
static FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib/fonts/font.bin")); // We'll need a font file

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub buffer_ptr: *mut u8,
}

impl Framebuffer {
    /// Initializes the framebuffer by sending a message to the GPU.
    pub fn new(width: u32, height: u32) -> Result<Self, &'static str> {
        // Message structure to send to the GPU
        let mut msg = MessageBuffer([
            35 * 4, // Total size of the message in bytes
            0,      // Request code

            // Tag: Set physical display width/height
            0x00048003, 8, 8, width, height,

            // Tag: Set virtual buffer width/height
            0x00048004, 8, 8, width, height,

            // Tag: Set virtual offset
            0x00048009, 8, 8, 0, 0,

            // Tag: Set depth
            0x00048005, 4, 4, 32, // 32 bits per pixel

            // Tag: Set pixel order (1 = RGB)
            0x00048006, 4, 4, 1,

            // Tag: Allocate buffer
            0x00040001, 8, 8, 0, 0, // Alignment and response placeholder

            // Tag: Get pitch
            0x00040008, 4, 4, 0, // Response placeholder

            0, // End tag
        ]);

        // Make the mailbox call
        if unsafe { mailbox_call(8, &mut msg.0 as *mut u32) } && msg.0[28] != 0 {
            Ok(Framebuffer {
                width: msg.0[5],
                height: msg.0[6],
                pitch: msg.0[33],
                // The GPU returns a bus address, which needs to be converted to a physical address.
                // For RPi3, bus address 0xCXXXXXXX corresponds to physical 0x3XXXXXXX.
                buffer_ptr: (msg.0[28] & 0x3FFFFFFF) as *mut u8,
            })
        } else {
            Err("Failed to initialize framebuffer")
        }
    }

    /// Draws a single character to the screen.
    pub fn put_char(&self, c: char, x: u32, y: u32, color: u32) {
        let char_offset = (c as usize) * 16; // Our font has 16 bytes per character
        let font_char = &FONT[char_offset..char_offset + 16];

        for row in 0..16 {
            let mut pixel_offset = self.buffer_ptr as u32 + (y + row) * self.pitch + x * 4;
            for col in 0..8 {
                if (font_char[row as usize] & (1 << col)) != 0 {
                    unsafe {
                        write_volatile(pixel_offset as *mut u32, color);
                    }
                }
                pixel_offset += 4; // Move to the next pixel (4 bytes for 32bpp)
            }
        }
    }

    /// Prints a string to the screen.
    pub fn print(&self, s: &str, mut x: u32, y: u32, color: u32) {
        for c in s.chars() {
            self.put_char(c, x, y, color);
            x += 8; // Advance x position for the next character
        }
    }
}