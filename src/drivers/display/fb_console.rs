// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! Framebuffer console that mirrors all output to the HDMI display.
//!
//! [`FbConsole`] implements both [`OutputDevice`] and (therefore) [`Console`]
//! via the blanket implementation in `kernel/console.rs`. It is constructed by
//! the BCM2708 framebuffer driver after the framebuffer has been mapped, then
//! registered with **both** [`register_console`] and [`register_output`] so
//! that:
//!
//! * `kprintln!` output (routed through `SYSTEM_CONSOLES`) appears on screen.
//! * [`SystemTerminal`] output — shell prompts, command output, key echo —
//!   also appears on screen, because `SystemTerminal::write_all` writes to
//!   every device in `OUTPUT_DEVICES`.
//!
//! # Text rendering
//!
//! Characters are rendered from the embedded 8×16 bitmap font in
//! [`super::font`]. The cursor advances left-to-right, wrapping at the right
//! edge. When the last row is reached a one-row scroll-up is performed via
//! [`DrawingContext::copy_region`] followed by a [`DrawingContext::fill_rect`]
//! to blank the new bottom row.
//!
//! # ANSI escape-sequence stripping
//!
//! The kernel terminal emits ANSI/VT sequences for cursor control and colour.
//! A lightweight state machine silently discards them so only printable text
//! is rendered:
//!
//! | State | Trigger | Next state |
//! |-------|---------|------------|
//! | Normal | `ESC` (0x1B) | Escape |
//! | Escape | `[` | Csi |
//! | Escape | `]` | Osc |
//! | Escape | anything else | Normal (2-char seq consumed) |
//! | Csi | `0x40–0x7E` (final byte) | Normal |
//! | Csi | anything else | Csi (parameter/intermediate) |
//! | Osc | `BEL` (0x07) | Normal |
//! | Osc | `ESC` | Escape (start of ST terminator) |
//! | Osc | anything else | Osc |
//!
//! # Byte handling (Normal state)
//!
//! | Byte | Action |
//! |------|--------|
//! | `0x20..=0x7E` | render glyph, advance column |
//! | `\n` (0x0A) | CR + advance row (scroll if needed) |
//! | `\r` (0x0D) | column → 0 |
//! | `\x08` (BS) | column--, erase cell |
//! | other | silently ignored |

use alloc::string::String;
use alloc::sync::Arc;

use crate::arch::cpu::memory_barrier;
use crate::drivers::{Device, DriverInitError};
use crate::kernel::console::register_console;
use crate::kernel::devicetree::node::Node;
use crate::kernel::sync::SpinLock;
use crate::kernel::terminal::{OutputDevice, register_output};

use super::drawing::DrawingContext;
use super::font::{FONT_HEIGHT, FONT_WIDTH, get_glyph};

const DEFAULT_FG: u32 = 0xFFFF_FFFF; // opaque white
const DEFAULT_BG: u32 = 0xFF00_0000; // opaque black

/// ANSI/VT escape-sequence parser state.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AnsiState {
    /// Normal text rendering.
    Normal,
    /// Saw `ESC` (0x1B); waiting for the sequence type byte.
    Escape,
    /// Inside a CSI sequence (`ESC [`); consuming until a final byte.
    Csi,
    /// Inside an OSC sequence (`ESC ]`); consuming until BEL or ST.
    Osc,
}

struct FbConsoleState {
    col: u32,
    row: u32,
    cols: u32,
    rows: u32,
    fb_width: u32,
    fb_height: u32,
    fg: u32,
    bg: u32,
    ansi_state: AnsiState,
}

pub struct FbConsole {
    id: String,
    ctx: DrawingContext,
    state: SpinLock<FbConsoleState>,
}

impl FbConsole {
    /// Creates a new `FbConsole` wrapping `fb` and immediately clears the screen.
    pub fn new(id: String, fb: Arc<dyn super::FrameBufferDevice>) -> Self {
        let width = fb.width();
        let height = fb.height();
        let cols = width / FONT_WIDTH;
        let rows = height / FONT_HEIGHT;

        let ctx = DrawingContext::new(fb);
        // Clear to background colour at construction time.
        ctx.fill_rect(0, 0, width, height, DEFAULT_BG);

        let state = FbConsoleState {
            col: 0,
            row: 0,
            cols,
            rows,
            fb_width: width,
            fb_height: height,
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            ansi_state: AnsiState::Normal,
        };

        Self {
            id,
            ctx,
            state: SpinLock::new(state),
        }
    }

    /// Scrolls the display up by one text row and clears the last row.
    fn scroll(ctx: &DrawingContext, s: &mut FbConsoleState) {
        // Shift all rows one row upward.
        ctx.copy_region(0, FONT_HEIGHT, 0, 0, s.fb_width, (s.rows - 1) * FONT_HEIGHT);
        // Clear the newly exposed bottom row.
        ctx.fill_rect(0, (s.rows - 1) * FONT_HEIGHT, s.fb_width, FONT_HEIGHT, s.bg);
        s.row = s.rows - 1;
    }

    /// Advances the cursor to the next row, scrolling if necessary.
    fn newline(ctx: &DrawingContext, s: &mut FbConsoleState) {
        s.col = 0;
        s.row += 1;
        if s.row >= s.rows {
            Self::scroll(ctx, s);
        }
    }
}

impl Device for FbConsole {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, _node: &Node) -> Result<(), DriverInitError> {
        // Register as an OutputDevice so SystemTerminal::write_all mirrors
        // shell/terminal output here, in addition to the UART.
        register_output(self.clone());
        // Register as a Console so kprintln! output also appears here.
        register_console(self);
        Ok(())
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl OutputDevice for FbConsole {
    fn write(&self, bytes: &[u8]) {
        // Use lock_irq so this is safe whether called from task context
        // (OUTPUT_DEVICES path) or from an IRQ-disabled context
        // (SYSTEM_CONSOLES.lock_irq path via console_write_str).
        let mut s = self.state.lock_irq();

        for &byte in bytes {
            match s.ansi_state {
                AnsiState::Escape => match byte {
                    b'[' => s.ansi_state = AnsiState::Csi,
                    b']' => s.ansi_state = AnsiState::Osc,
                    0x1B => {} // consecutive ESC; stay in Escape
                    _ => s.ansi_state = AnsiState::Normal, // 2-char seq complete
                },

                AnsiState::Csi => {
                    // Final byte of a CSI sequence is in 0x40–0x7E.
                    if (0x40..=0x7E).contains(&byte) {
                        s.ansi_state = AnsiState::Normal;
                    }
                    // Parameter/intermediate bytes are silently consumed.
                }

                AnsiState::Osc => match byte {
                    0x07 => s.ansi_state = AnsiState::Normal,   // BEL terminates
                    0x1B => s.ansi_state = AnsiState::Escape,   // ST = ESC '\'
                    _ => {}
                },

                AnsiState::Normal => match byte {
                    0x1B => s.ansi_state = AnsiState::Escape,

                    0x20..=0x7E => {
                        // Printable ASCII: blit glyph then advance cursor.
                        let px = s.col * FONT_WIDTH;
                        let py = s.row * FONT_HEIGHT;
                        let bitmap = get_glyph(byte as char);
                        self.ctx.draw_glyph(px, py, bitmap, s.fg, s.bg);

                        s.col += 1;
                        if s.col >= s.cols {
                            Self::newline(&self.ctx, &mut s);
                        }
                    }
                    b'\n' => {
                        Self::newline(&self.ctx, &mut s);
                    }
                    b'\r' => {
                        s.col = 0;
                    }
                    0x08 => {
                        // Backspace: move cursor back and erase the cell.
                        if s.col > 0 {
                            s.col -= 1;
                        }
                        let px = s.col * FONT_WIDTH;
                        let py = s.row * FONT_HEIGHT;
                        self.ctx.fill_rect(px, py, FONT_WIDTH, FONT_HEIGHT, s.bg);
                    }
                    _ => {} // other control characters silently ignored
                },
            }
        }

        // One memory barrier after all pixel writes for this call.
        memory_barrier();
    }
}
