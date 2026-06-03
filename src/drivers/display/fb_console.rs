// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>
//
// @vibe-coded

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
//! edge. When the last row is reached a one-row scroll-up is performed via a
//! DMA 2D copy when available, otherwise via [`DrawingContext::copy_region`],
//! followed by a [`DrawingContext::fill_rect`] to blank the new bottom row.
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
use core::sync::atomic::{AtomicU32, Ordering};

use crate::arch::cpu::memory_barrier;
use crate::drivers::dma_controller::bcm2835_dma;
use crate::drivers::dma_controller::{Dma2DTransfer, DmaError, get_dma_engine};
use crate::drivers::{Device, DriverInitError};
use crate::kernel::console::register_console;
use crate::kernel::devicetree::node::Node;
use crate::kernel::shell::{ShellCommand, register_command};
use crate::kernel::sync::{OnceLock, SpinLock};
use crate::kernel::terminal::get_system_terminal;
use crate::kernel::terminal::{OutputDevice, register_output};

use super::drawing::DrawingContext;
use super::font::{FONT_HEIGHT, FONT_WIDTH, get_glyph};

const DEFAULT_FG: u32 = 0xFFFF_FFFF; // opaque white
const DEFAULT_BG: u32 = 0xFF00_0000; // opaque black
const FB_DMA_ERROR_NONE: u32 = 0;
const FB_DMA_ERROR_BUSY: u32 = 1;
const FB_DMA_ERROR_UNAVAILABLE: u32 = 2;
const FB_DMA_ERROR_INVALID: u32 = 3;
const FB_DMA_ERROR_TIMEOUT: u32 = 4;
const FB_DMA_ERROR_FAULT: u32 = 5;

static FB_SCROLLS: AtomicU32 = AtomicU32::new(0);
static FB_DMA_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
static FB_DMA_SUCCESSES: AtomicU32 = AtomicU32::new(0);
static FB_DMA_NO_ENGINE_FALLBACKS: AtomicU32 = AtomicU32::new(0);
static FB_DMA_BUSY_FALLBACKS: AtomicU32 = AtomicU32::new(0);
static FB_DMA_ERROR_FALLBACKS: AtomicU32 = AtomicU32::new(0);
static FB_DMA_DISABLED_FALLBACKS: AtomicU32 = AtomicU32::new(0);
static FB_LAST_DMA_ERROR: AtomicU32 = AtomicU32::new(FB_DMA_ERROR_NONE);
static FB_STATS_COMMAND_REGISTERED: OnceLock<()> = OnceLock::new();

fn fb_dma_error_name(code: u32) -> &'static str {
    match code {
        FB_DMA_ERROR_BUSY => "busy",
        FB_DMA_ERROR_UNAVAILABLE => "no-engine",
        FB_DMA_ERROR_INVALID => "invalid",
        FB_DMA_ERROR_TIMEOUT => "timeout",
        FB_DMA_ERROR_FAULT => "fault",
        _ => "none",
    }
}

fn reset_fb_stats() {
    FB_SCROLLS.store(0, Ordering::Relaxed);
    FB_DMA_ATTEMPTS.store(0, Ordering::Relaxed);
    FB_DMA_SUCCESSES.store(0, Ordering::Relaxed);
    FB_DMA_NO_ENGINE_FALLBACKS.store(0, Ordering::Relaxed);
    FB_DMA_BUSY_FALLBACKS.store(0, Ordering::Relaxed);
    FB_DMA_ERROR_FALLBACKS.store(0, Ordering::Relaxed);
    FB_DMA_DISABLED_FALLBACKS.store(0, Ordering::Relaxed);
    FB_LAST_DMA_ERROR.store(FB_DMA_ERROR_NONE, Ordering::Relaxed);
}

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
    dma_failed: bool,
}

pub struct FbConsole {
    id: String,
    ctx: DrawingContext,
    state: SpinLock<FbConsoleState>,
}

impl FbConsole {
    fn register_stats_command() {
        if FB_STATS_COMMAND_REGISTERED.set(()).is_err() {
            return;
        }

        register_command(ShellCommand::new(
            "fbstats",
            "Show framebuffer scroll and DMA stats (-r resets)",
            |_shell, args| {
                if let Some(terminal) = get_system_terminal() {
                    if args.contains(&"-r") {
                        reset_fb_stats();
                        bcm2835_dma::reset_stats();
                        terminal.write(b"fbstats reset\n");
                        return;
                    }

                    let dma_stats = bcm2835_dma::get_stats();
                    let message = alloc::format!(
                        "fb: scrolls={} dma_attempts={} dma_ok={} no_engine={} busy={} error={} disabled={} last={}\ndma: started={} irq_ok={} poll_ok={} timeouts={} faults={} busy={} invalid={} last_cs=0x{:08x} last_dbg=0x{:08x} last={}\n",
                        FB_SCROLLS.load(Ordering::Relaxed),
                        FB_DMA_ATTEMPTS.load(Ordering::Relaxed),
                        FB_DMA_SUCCESSES.load(Ordering::Relaxed),
                        FB_DMA_NO_ENGINE_FALLBACKS.load(Ordering::Relaxed),
                        FB_DMA_BUSY_FALLBACKS.load(Ordering::Relaxed),
                        FB_DMA_ERROR_FALLBACKS.load(Ordering::Relaxed),
                        FB_DMA_DISABLED_FALLBACKS.load(Ordering::Relaxed),
                        fb_dma_error_name(FB_LAST_DMA_ERROR.load(Ordering::Relaxed)),
                        dma_stats.transfers_started,
                        dma_stats.irq_completions,
                        dma_stats.poll_completions,
                        dma_stats.timeouts,
                        dma_stats.faults,
                        dma_stats.busy_errors,
                        dma_stats.invalid_errors,
                        dma_stats.last_cs,
                        dma_stats.last_debug,
                        match dma_stats.last_error {
                            1 => "timeout",
                            2 => "fault",
                            3 => "invalid",
                            4 => "busy",
                            _ => "none",
                        },
                    );
                    terminal.write(message.as_bytes());
                }
            },
        ));
    }

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
            dma_failed: false,
        };

        Self {
            id,
            ctx,
            state: SpinLock::new(state),
        }
    }

    /// Scrolls the display up by one text row and clears the last row.
    fn scroll(&self, s: &mut FbConsoleState) {
        FB_SCROLLS.fetch_add(1, Ordering::Relaxed);
        let row_len_bytes = s.fb_width * 4;
        let row_count = (s.rows - 1) * FONT_HEIGHT;
        let mut used_dma = false;

        if s.dma_failed {
            FB_DMA_DISABLED_FALLBACKS.fetch_add(1, Ordering::Relaxed);
        } else {
            if let Some(dma) = get_dma_engine() {
                FB_DMA_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
                let transfer = Dma2DTransfer {
                    src_bus_addr: self.ctx.bus_addr() + (FONT_HEIGHT * self.ctx.pitch()),
                    dst_bus_addr: self.ctx.bus_addr(),
                    row_len_bytes,
                    row_count,
                    src_stride: self.ctx.pitch() - row_len_bytes,
                    dst_stride: self.ctx.pitch() - row_len_bytes,
                };

                match dma.copy_2d(transfer) {
                    Ok(()) => {
                        used_dma = true;
                        FB_DMA_SUCCESSES.fetch_add(1, Ordering::Relaxed);
                        FB_LAST_DMA_ERROR.store(FB_DMA_ERROR_NONE, Ordering::Relaxed);
                    }
                    Err(DmaError::Busy) => {
                        FB_DMA_BUSY_FALLBACKS.fetch_add(1, Ordering::Relaxed);
                        FB_LAST_DMA_ERROR.store(FB_DMA_ERROR_BUSY, Ordering::Relaxed);
                    }
                    Err(DmaError::Unavailable) => {
                        FB_DMA_NO_ENGINE_FALLBACKS.fetch_add(1, Ordering::Relaxed);
                        FB_LAST_DMA_ERROR.store(FB_DMA_ERROR_UNAVAILABLE, Ordering::Relaxed);
                    }
                    Err(err @ (DmaError::InvalidTransfer | DmaError::Timeout | DmaError::Fault)) => {
                        FB_DMA_ERROR_FALLBACKS.fetch_add(1, Ordering::Relaxed);
                        FB_LAST_DMA_ERROR.store(
                            match err {
                                DmaError::InvalidTransfer => FB_DMA_ERROR_INVALID,
                                DmaError::Timeout => FB_DMA_ERROR_TIMEOUT,
                                DmaError::Fault => FB_DMA_ERROR_FAULT,
                                _ => FB_DMA_ERROR_NONE,
                            },
                            Ordering::Relaxed,
                        );
                        s.dma_failed = true;
                    }
                }
            } else {
                FB_DMA_NO_ENGINE_FALLBACKS.fetch_add(1, Ordering::Relaxed);
                FB_LAST_DMA_ERROR.store(FB_DMA_ERROR_UNAVAILABLE, Ordering::Relaxed);
            }
        }

        if !used_dma {
            // Shift all rows one row upward.
            self.ctx
                .copy_region(0, FONT_HEIGHT, 0, 0, s.fb_width, row_count);
        }

        // Clear the newly exposed bottom row.
        self.ctx
            .fill_rect(0, (s.rows - 1) * FONT_HEIGHT, s.fb_width, FONT_HEIGHT, s.bg);
        s.row = s.rows - 1;
    }

    /// Advances the cursor to the next row, scrolling if necessary.
    fn newline(&self, s: &mut FbConsoleState) {
        s.col = 0;
        s.row += 1;
        if s.row >= s.rows {
            self.scroll(s);
        }
    }
}

impl Device for FbConsole {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, _node: &Node) -> Result<(), DriverInitError> {
        Self::register_stats_command();
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
                            self.newline(&mut s);
                        }
                    }
                    b'\n' => {
                        self.newline(&mut s);
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
