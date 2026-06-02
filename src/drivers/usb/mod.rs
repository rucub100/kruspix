// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;

use crate::kernel::sync::OnceLock;

pub mod core;
pub mod bcm2708_usb;

pub use core::*;

static USB_HOST_CONTROLLER: OnceLock<Arc<dyn UsbHostController>> = OnceLock::new();

pub fn register_host_controller(controller: Arc<dyn UsbHostController>) -> Result<(), ()> {
    USB_HOST_CONTROLLER.set(controller).map_err(|_| ())
}

pub fn get_host_controller() -> Option<Arc<dyn UsbHostController>> {
    USB_HOST_CONTROLLER.get().cloned()
}
