#[cfg(target_arch = "aarch64")]
#[path = "arm64/kernel/mod.rs"]
pub mod kernel;

#[cfg(target_arch = "aarch64")]
#[path = "arm64/mm/mod.rs"]
pub mod mm;
