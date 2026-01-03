// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::ops::DerefMut;

pub trait TableDescriptorAttributes {
    const NS_TABLE: u64 = 1 << 63;
    const AP_TABLE: u64 = 11 << 61;
    const XN_TABLE: u64 = 1 << 60;
    const PXN_TABLE: u64 = 1 << 59;
}

pub trait BlockAndPageDescriptorAttributes: DerefMut<Target = u64> + Sized {
    const EXECUTE_NEVER: u64 = 1 << 54;
    const ACCESS_FLAG: u64 = 1 << 10;
    const SHAREABILITY: u64 = 3 << 8;
    const ATTR_INDEX: u64 = 7 << 2;

    fn set_execute_never(&mut self, xn: bool) -> &mut Self {
        if xn {
            **self |= Self::EXECUTE_NEVER;
        } else {
            **self &= !Self::EXECUTE_NEVER;
        }

        self
    }

    fn set_accessed(&mut self, accessed: bool) -> &mut Self {
        if accessed {
            **self |= Self::ACCESS_FLAG;
        } else {
            **self &= !Self::ACCESS_FLAG;
        }
        self
    }

    fn set_shareability(&mut self, attr: ShareabilityAttribute) -> &mut Self {
        **self &= !Self::SHAREABILITY;
        **self |= (attr as u64) << 8;
        self
    }

    fn set_mem_attr_index(&mut self, index: MemoryRegionAttrIndex) -> &mut Self {
        **self &= !Self::ATTR_INDEX;
        **self |= (index as u64) << 2;
        self
    }
}

pub enum ShareabilityAttribute {
    NonShareable = 0b00,
    OuterShareable = 0b10,
    InnerShareable = 0b11,
}

/// Memory region attribute index for `MAIR_EL1` register.
///
/// # Safety
/// This enum is derived from the `MAIR_EL1` register configuration.
/// The configuration must match the indices used here.
/// See [`crate::arch::kernel::entry`] for more details.
pub enum MemoryRegionAttrIndex {
    DeviceNgnRnE = 0b000,
    NormalNonCacheable = 0b001,
    NormalWriteBackNonTransientReadWriteAlloc = 0b010,
}
