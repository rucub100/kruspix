// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::string::String;
use alloc::sync::Arc;

use crate::arch::cpu::memory_barrier;
use crate::drivers::syscon::{FramebufferInfo, get_rpi_firmware};
use crate::drivers::{DEVICE_MANAGER, Device, DriverInitError, DriverRegistry, PlatformDriver};
use crate::kernel::devicetree::node::Node;
use crate::kernel::devicetree::prop::PropertyValue;
use crate::kernel::devicetree::{PHandle, get_devicetree};
use crate::kprintln;
use crate::mm::map_io_region;

use super::{FrameBufferDevice, register_framebuffer};

/// Strips the VideoCore bus alias bits from a firmware-reported framebuffer address.
///
/// On Pi 3 the VC GPU returns addresses in the `0xC000_0000` L2-cached alias
/// range. The ARM CPU must strip those bits before passing the address to
/// `map_io_region()`.
#[inline]
fn strip_vc_bus_alias(bus_addr: u32) -> usize {
    (bus_addr & 0x3FFF_FFFF) as usize
}

struct Bcm2708Fb {
    id: String,
    /// Mapped virtual address of the framebuffer, returned by `map_io_region()`.
    fb_va: usize,
    width: u32,
    height: u32,
    pitch: u32,
    depth: u32,
}

impl Bcm2708Fb {
    fn new(id: String, fb_va: usize, info: &FramebufferInfo) -> Self {
        Self {
            id,
            fb_va,
            width: info.width,
            height: info.height,
            pitch: info.pitch,
            depth: info.depth,
        }
    }
}

impl Device for Bcm2708Fb {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn global_setup(self: Arc<Self>, _node: &Node) -> Result<(), DriverInitError> {
        register_framebuffer(self).map_err(|_| DriverInitError::DeviceFailed)
    }

    fn local_setup(self: Arc<Self>) -> Result<(), DriverInitError> {
        Ok(())
    }
}

impl FrameBufferDevice for Bcm2708Fb {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn pitch(&self) -> u32 {
        self.pitch
    }

    fn depth_bpp(&self) -> u32 {
        self.depth
    }

    fn fill(&self, color: u32) {
        for y in 0..self.height {
            let row_start_va = self.fb_va + (y * self.pitch) as usize;
            let row_ptr = row_start_va as *mut u32;

            for x in 0..self.width {
                // SAFETY: Invariants validated in try_init:
                //   - depth == 32: 4 bytes per pixel; casting fb_va offset to *mut u32 is correct.
                //   - width * 4 <= pitch: writing x < width pixels stays within the row stride.
                //   - height * pitch <= size: y < height ensures row_start_va is within the mapped region.
                unsafe {
                    core::ptr::write_volatile(row_ptr.add(x as usize), color);
                }
            }
        }

        memory_barrier();
    }

    fn write_pixel(&self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let row_start_va = self.fb_va + (y * self.pitch) as usize;
        let row_ptr = row_start_va as *mut u32;

        // SAFETY: x < self.width and y < self.height are checked above. Invariants validated in
        //   try_init: depth == 32 (4 bytes/pixel, *mut u32 cast is correct), width * 4 <= pitch
        //   (pixel write stays within row stride), height * pitch <= size (row_start_va is
        //   within the mapped region).
        unsafe {
            core::ptr::write_volatile(row_ptr.add(x as usize), color);
        }
    }

    fn buffer_ptr(&self) -> *mut u8 {
        self.fb_va as *mut u8
    }
}

pub struct Bcm2708FbDriver {
    dev_registry: DriverRegistry<Bcm2708Fb>,
}

impl Bcm2708FbDriver {
    pub const fn new() -> Self {
        Self {
            dev_registry: DriverRegistry::new(),
        }
    }
}

impl PlatformDriver for Bcm2708FbDriver {
    fn compatible(&self) -> &[&str] {
        &["brcm,bcm2708-fb"]
    }

    fn try_init(&self, node: &Node) -> Result<(), DriverInitError> {
        // Parse the `firmware` phandle property from the device tree node.
        let firmware_phandle: PHandle = node
            .properties()
            .iter()
            .find(|p| p.name() == "firmware")
            .and_then(|p| match p.value() {
                PropertyValue::Unknown(prop) => prop.try_into().ok(),
                _ => None,
            })
            .ok_or(DriverInitError::DeviceTreeError)?;

        let dt = get_devicetree().ok_or(DriverInitError::DeviceFailed)?;
        let fw_node = dt
            .node_by_phandle(&firmware_phandle)
            .ok_or(DriverInitError::DeviceFailed)?;

        // Wait until the firmware device is registered before proceeding.
        let _ = DEVICE_MANAGER
            .get_device(fw_node.path().as_str())
            .ok_or(DriverInitError::Retry)?;

        let firmware = get_rpi_firmware().ok_or(DriverInitError::DeviceFailed)?;

        let (width, height) = firmware.get_preferred_resolution().unwrap_or((1920, 1080));
        kprintln!("[bcm2708-fb] resolution: {}x{}", width, height);

        let info = firmware
            .init_framebuffer(width, height, 32)
            .map_err(|_| DriverInitError::DeviceFailed)?;

        if info.depth != 32 {
            kprintln!("[bcm2708-fb] [ERROR] unsupported framebuffer depth: {} bpp", info.depth);
            return Err(DriverInitError::DeviceFailed);
        }

        if info.width * 4 > info.pitch
            || info.height.checked_mul(info.pitch).map_or(true, |total| total > info.size)
        {
            kprintln!(
                "[bcm2708-fb] [ERROR] inconsistent framebuffer geometry: {}x{} pitch={} size={}",
                info.width,
                info.height,
                info.pitch,
                info.size,
            );
            return Err(DriverInitError::DeviceFailed);
        }

        kprintln!(
            "[bcm2708-fb] framebuffer: bus_addr=0x{:08x}, size={} bytes, pitch={}",
            info.bus_addr,
            info.size,
            info.pitch,
        );

        let phys_addr = strip_vc_bus_alias(info.bus_addr);
        let fb_va = map_io_region(phys_addr, info.size as usize);

        let dev = Arc::new(Bcm2708Fb::new(node.path(), fb_va, &info));

        dev.clone().global_setup(node)?;

        self.dev_registry.add_device(node.path(), dev);

        Ok(())
    }

    fn get_device(&self, id: &str) -> Option<Arc<dyn Device>> {
        self.dev_registry.get_device_opaque(id)
    }
}

pub static DRIVER: Bcm2708FbDriver = Bcm2708FbDriver::new();
