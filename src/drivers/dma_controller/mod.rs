// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;

use crate::kernel::sync::OnceLock;

use super::Device;

pub mod bcm2835_dma;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaError {
    Unavailable,
    Busy,
    InvalidTransfer,
    Timeout,
    Fault,
}

#[derive(Debug, Clone, Copy)]
pub struct Dma2DTransfer {
    pub src_bus_addr: u32,
    pub dst_bus_addr: u32,
    pub row_len_bytes: u32,
    pub row_count: u32,
    pub src_stride: u32,
    pub dst_stride: u32,
}

pub trait DmaEngine: Device {
    fn copy_2d(&self, transfer: Dma2DTransfer) -> Result<(), DmaError>;
}

static SYSTEM_DMA_ENGINE: OnceLock<Arc<dyn DmaEngine>> = OnceLock::new();

pub fn register_dma_engine(engine: Arc<dyn DmaEngine>) -> Result<(), ()> {
    SYSTEM_DMA_ENGINE.set(engine).map_err(|_| ())
}

pub fn get_dma_engine() -> Option<Arc<dyn DmaEngine>> {
    SYSTEM_DMA_ENGINE.get().cloned()
}