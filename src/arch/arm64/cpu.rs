// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::arch::asm;

#[inline(always)]
pub fn core_id() -> usize {
    let mpidr: u64;
    unsafe { asm!("mrs {}, mpidr_el1", out(reg) mpidr, options(nomem, nostack)) };
    (mpidr & 0xff) as usize
}

/// Disable all interrupts and return the `handle` for the [`local_restore_interrupts`] function.
#[inline(always)]
pub fn local_disable_interrupts() -> usize {
    let daif: usize;
    unsafe {
        asm!(
            "mrs {}, daif",
            "msr daifset, #0b1111",
            out(reg) daif,
            options(nostack)
        )
    };
    daif
}

/// Disable IRQ and FIQ interrupts and return the `handle` for the [`local_restore_interrupts`] function.
#[inline(always)]
pub fn local_disable_irq_fiq() -> usize {
    let daif: usize;
    unsafe {
        asm!(
        "mrs {}, daif",
        "msr daifset, #0b11",
        out(reg) daif,
        options(nostack)
        )
    };
    daif
}

/// Restore the interrupt flags from the given `handle` value.
///
/// # Safety
/// You must ensure that the `handle` value was obtained from a previous call to [`local_disable_interrupts`] or [`local_disable_irq_fiq`].
#[inline(always)]
pub unsafe fn local_restore_interrupts(handle: usize) {
    unsafe {
        asm!(
            "msr daif, {}",
            in(reg) handle,
            options(nostack)
        )
    };
}

/// Enable IRQ and FIQ interrupts on the current core.
#[inline(always)]
pub fn local_enable_irq_fiq() {
    unsafe { asm!("msr daifclr, #0b11", options(nomem, nostack)) };
}

/// Returns the raw pointer to the current core's local storage.
///
/// # Safety
/// The caller must ensure that the returned reference is valid.
#[inline(always)]
pub(crate) unsafe fn get_local<T>() -> &'static T {
    let val: usize;
    unsafe {
        asm!(
            "mrs {0}, tpidr_el1",
            out(reg) val,
            options(nomem, nostack, preserves_flags)
        );

        &*(val as *const T)
    }
}

/// Sets the raw pointer for the current core's local storage.
pub(crate) unsafe fn set_local<T>(data: &T) {
    let ptr = data as *const T as usize;

    unsafe {
        asm!("msr tpidr_el1, {0}", in(reg) ptr, options(nomem, nostack, preserves_flags));
    }
}
