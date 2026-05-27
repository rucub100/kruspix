// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! Extended Display Identification Data (EDID) — VESA standard parsing.
//!
//! EDID is a 128-byte (base block) metadata structure that a display exposes
//! to the host, describing its identity, supported resolutions, refresh rates,
//! and timing parameters. Enhanced EDID (E-EDID) extends this with one or more
//! 128-byte extension blocks (e.g., CEA-861 for HDMI audio/video capabilities).
//!
//! This module provides:
//! - [`EdidBlock`] — a parsed representation of the 128-byte base EDID block.
//! - Validation of the fixed EDID header and checksum.
//! - Extraction of preferred timing (first detailed timing descriptor).
//!
//! This code is hardware-agnostic. Any display driver that can retrieve raw
//! EDID bytes (e.g., via DDC/I²C or firmware mailbox) can use it.

