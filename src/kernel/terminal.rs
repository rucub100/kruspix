// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! TODO: TTY module shall be implemented here
//!
//! Strategy:
//!
//! We need to classify input-only devices (keyboards, mice, touchscreens) and
//! output-only devices (framebuffers, etc.) in order to compose TTYs from them.
//!
//! However, if a device is both input and output (like serial consoles), we shall
//! keep them together in a terminal instance.
//!
//! For the primary console, we shall pick the first available terminal device found
//! or derive it from the stdout-path property in the devicetree (/chosen node).
//!
//! If the primary console happens to be both input and output, we can use it as the "primary terminal".
//! Otherwise, we shall try to compose the terminal from the primary output device and an
//! appropriate input-only device (keyboard, etc.). The input device selection strategy may follow the same
//! principles in this case: check the stdin-path first or fallback to the first input-only device found.
//!
//! Secondary terminals may be created from devices that are not used yes and are both input and output capable
//! or by composing input-only and output-only devices together according to some logic
//! (like matching local keyboards with framebuffers).

use crate::drivers::Device;

pub trait Input: Device + Send + Sync {
    // TODO: define input methods
    fn read(&self);
}

pub trait Terminal: Send + Sync {
    // TODO: define terminal methods
}