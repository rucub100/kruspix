// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;

use crate::kernel::sync::OnceLock;

pub mod bcm2835_firmware;

pub struct FramebufferInfo {
    pub bus_addr: u32,
    pub size: u32,
    pub pitch: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

/// Abstraction over the RPi VideoCore firmware for display/framebuffer operations.
pub trait SystemFirmware: Send + Sync {
    fn get_preferred_resolution(&self) -> Option<(u32, u32)>;

    fn init_framebuffer(&self, width: u32, height: u32, depth: u32) -> Result<FramebufferInfo, ()>;
}

static SYSTEM_FIRMWARE: OnceLock<Arc<dyn SystemFirmware>> = OnceLock::new();

pub fn register_rpi_firmware(fw: Arc<dyn SystemFirmware>) -> Result<(), ()> {
    SYSTEM_FIRMWARE.set(fw).map_err(|_| ())
}

pub fn get_rpi_firmware() -> Option<Arc<dyn SystemFirmware>> {
    SYSTEM_FIRMWARE.get().cloned()
}