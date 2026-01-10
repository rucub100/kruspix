// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::sync::Arc;

use crate::drivers::Device;
use crate::kernel::sync::OnceLock;

pub enum RngError {
    HardwareError,
    Timeout,
    AlreadyRegistered,
}

pub type RngResult<T> = Result<T, RngError>;

pub trait RandomNumberGenerator: Device {
    fn name(&self) -> &str;
    fn enable(&self) -> RngResult<()>;
    fn disable(&self) -> RngResult<()>;
    fn read(&self, buffer: &mut [u8], wait: bool) -> RngResult<usize>;

    fn read_exact(&self, buffer: &mut [u8]) -> RngResult<()> {
        let mut offset = 0;
        while offset < buffer.len() {
            let read = self.read(&mut buffer[offset..], true)?;

            if read == 0 {
                return Err(RngError::Timeout);
            }

            offset += read;
        }

        Ok(())
    }

    fn next_u32(&self) -> RngResult<u32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    fn next_u64(&self) -> RngResult<u64> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    fn next_usize(&self) -> RngResult<usize> {
        let mut buf = [0u8; size_of::<usize>()];
        self.read_exact(&mut buf)?;
        Ok(usize::from_le_bytes(buf))
    }
}

static RNG: OnceLock<Arc<dyn RandomNumberGenerator>> = OnceLock::new();

pub fn register_rng(rng: Arc<dyn RandomNumberGenerator>) -> RngResult<()> {
    RNG.set(rng).map_err(|_| RngError::AlreadyRegistered)
}

pub fn get_rng() -> Option<Arc<dyn RandomNumberGenerator>> {
    RNG.get().cloned()
}
