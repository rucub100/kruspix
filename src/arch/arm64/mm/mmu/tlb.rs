// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::arch::asm;

use super::PAGE_SHIFT;

#[inline(always)]
pub unsafe fn invalidate_all(user_table_pa: usize, kernel_table_pa: usize) {
    unsafe {
        asm!("dsb ishst");
        asm!("msr ttbr0_el1, {}", in(reg) user_table_pa);
        asm!("msr ttbr1_el1, {}", in(reg) kernel_table_pa);
        asm!("tlbi vmalle1");
        asm!("dsb ish");
        asm!("isb");
    }
}

#[inline(always)]
pub unsafe fn invalidate_by_va_all_asid_inner_shareable(va: usize) {
    unsafe {
        asm!("dsb ishst");
        asm!("tlbi vaae1is, {}", in(reg) va >> PAGE_SHIFT);
        asm!("dsb ish");
        asm!("isb");
    }
}

#[inline(always)]
pub unsafe fn tlb_invalidate_last_level_by_va_all_asid_inner_shareable(va: usize) {
    unsafe {
        asm!("dsb ishst");
        asm!("tlbi vaale1is, {}", in(reg) va >> PAGE_SHIFT);
        asm!("dsb ish");
        asm!("isb");
    }
}
