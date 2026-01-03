// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

#[cfg(target_arch = "aarch64")]
#[path = "arm64/kernel/mod.rs"]
pub mod kernel;

#[cfg(target_arch = "aarch64")]
#[path = "arm64/mm/mod.rs"]
pub mod mm;

#[cfg(target_arch = "aarch64")]
#[path = "arm64/cpu.rs"]
pub mod cpu;
