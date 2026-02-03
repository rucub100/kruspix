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

#[derive(Default)]
#[repr(C)]
pub(crate) struct ArchContext {
    x19_x30: [usize; 12],
    stack_pointer: usize,
    daif: usize,
    // TODO: add more registers as needed (e.g. elr_el1, spsr_el1, etc.)
}

impl ArchContext {
    pub fn new<const N: usize>(stack: &[u8; N], entry: fn(), cleanup: fn()) -> Self {
        let mut x19_x30 = [0usize; 12];

        x19_x30[0] = entry as usize;
        x19_x30[1] = cleanup as usize;
        x19_x30[11] = entry_trampoline as *const () as usize;

        Self {
            x19_x30,
            // align stack pointer to 16 bytes
            stack_pointer: (stack.as_ptr_range().end as usize) & !0xF,
            daif: 0,
        }
    }
}

extern "C" fn entry_trampoline() -> ! {
    let entry_fn: fn();
    let cleanup_fn: fn();

    unsafe {
        asm!(
        "mov {}, x19",
        "mov {}, x20",
        out(reg) entry_fn,
        out(reg) cleanup_fn,
        );
    }

    entry_fn();
    cleanup_fn();

    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

pub(crate) unsafe fn switch_context(old: &mut ArchContext, new: &ArchContext) -> ! {
    unsafe {
        asm!(
            // save callee-saved registers x19-x30 into old context
            "stp x19, x20, [{old}, #16 * 0]",
            "stp x21, x22, [{old}, #16 * 1]",
            "stp x23, x24, [{old}, #16 * 2]",
            "stp x25, x26, [{old}, #16 * 3]",
            "stp x27, x28, [{old}, #16 * 4]",
            "stp x29, x30, [{old}, #16 * 5]",
            // save stack pointer into old context
            "mov {old_sp}, sp",
            // save daif into old context
            "mrs {old_daif}, daif",
            // ================================================================
            // restore daif from new context
            "msr daif, {new_daif}",
            // load stack pointer from new context
            "mov sp, {new_sp}",
            // restore callee-saved registers x19-x30 from new context
            "ldp x19, x20, [{new}, #16 * 0]",
            "ldp x21, x22, [{new}, #16 * 1]",
            "ldp x23, x24, [{new}, #16 * 2]",
            "ldp x25, x26, [{new}, #16 * 3]",
            "ldp x27, x28, [{new}, #16 * 4]",
            "ldp x29, x30, [{new}, #16 * 5]",
            "ret",
            old = in(reg) old,
            old_sp = out(reg) old.stack_pointer,
            old_daif = out(reg) old.daif,
            new_daif = in(reg) new.daif,
            new_sp = in(reg) new.stack_pointer,
            new = in(reg) new,
        )
    }

    unreachable!()
}

pub(crate) fn idle_task() {
    loop {
        // Idle task does nothing, just waits for interrupts
        unsafe {
            asm!("wfi", options(nomem, nostack, preserves_flags));
        }
    }
}
