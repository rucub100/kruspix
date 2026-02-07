// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

use core::arch::{asm, naked_asm};

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

#[repr(C)]
pub(crate) struct ArchContext {
    x19_x30: [usize; 12], // offset 0
    stack_pointer: usize, // offset 96 (12 * 8)
    daif: usize,          // offset 104 (96 + 8)
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
            stack_pointer: ((stack.as_ptr_range().end as usize) - 16) & !0xF,
            daif: 0,
        }
    }
}

#[unsafe(naked)]
extern "C" fn entry_trampoline() {
    naked_asm!(
        "blr x19",
        "blr x20",
        "1:",
        "wfi",
        "b 1b",
    );
}

#[unsafe(naked)]
pub(crate) extern "C" fn load_context(_new: &ArchContext) -> ! {
    naked_asm!(
        // load stack pointer from new context
        "ldr x9, [x0, #96]",
        "mov sp, x9",
        // restore callee-saved registers x19-x30 from new context
        "ldp x19, x20, [x0, #16 * 0]",
        "ldp x21, x22, [x0, #16 * 1]",
        "ldp x23, x24, [x0, #16 * 2]",
        "ldp x25, x26, [x0, #16 * 3]",
        "ldp x27, x28, [x0, #16 * 4]",
        "ldp x29, x30, [x0, #16 * 5]",
        // sync barrier
        "isb",
        // restore daif from new context
        "ldr x9, [x0, #104]",
        "msr daif, x9",
        // return to the restored context (x30 is the return address)
        "ret",
    )
}

#[unsafe(naked)]
pub(crate) extern "C" fn switch_context(_old: &ArchContext, _new: &ArchContext) {
    naked_asm!(
        // save callee-saved registers x19-x30 into old context
        "stp x19, x20, [x0, #16 * 0]",
        "stp x21, x22, [x0, #16 * 1]",
        "stp x23, x24, [x0, #16 * 2]",
        "stp x25, x26, [x0, #16 * 3]",
        "stp x27, x28, [x0, #16 * 4]",
        "stp x29, x30, [x0, #16 * 5]",
        // save stack pointer into old context
        "mov x9, sp",
        "str x9, [x0, #96]",
        // save daif into old context
        "mrs x9, daif",
        "str x9, [x0, #104]",
        // ================================================================
        // load stack pointer from new context
        "ldr x9, [x1, #96]",
        "mov sp, x9",
        // restore callee-saved registers x19-x30 from new context
        "ldp x19, x20, [x1, #16 * 0]",
        "ldp x21, x22, [x1, #16 * 1]",
        "ldp x23, x24, [x1, #16 * 2]",
        "ldp x25, x26, [x1, #16 * 3]",
        "ldp x27, x28, [x1, #16 * 4]",
        "ldp x29, x30, [x1, #16 * 5]",
        // sync barrier
        "isb",
        // restore daif from new context
        "ldr x9, [x1, #104]",
        "msr daif, x9",
        // return to the restored context (x30 is the return address)
        "ret",
    )
}

pub(crate) fn idle_task() {
    loop {
        wait_for_interrupt();
    }
}

#[inline(always)]
pub fn wait_for_interrupt() {
    unsafe {
        asm!("wfi", "isb", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn wait_for_event() {
    unsafe {
        asm!("wfe", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn send_event() {
    unsafe {
        asm!("sev", options(nomem, nostack, preserves_flags));
    }
}