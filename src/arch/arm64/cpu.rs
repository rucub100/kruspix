use core::arch::asm;

#[inline(always)]
pub fn core_id() -> usize {
    let mpidr: u64;
    unsafe { asm!("mrs {}, mpidr_el1", out(reg) mpidr, options(nomem, nostack)) };
    (mpidr & 0xff) as usize
}

/// Disable all interrupts and return the `handle` for the [`restore_interrupts`] function.
#[inline(always)]
pub fn disable_interrupts() -> usize {
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

/// Disable IRQ and FIQ interrupts and return the `handle` for the [`restore_interrupts`] function.
#[inline(always)]
pub fn disable_irq_fiq() -> usize {
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
/// You must ensure that the `handle` value was obtained from a previous call to [`disable_interrupts`] or [`disable_irq_fiq`].
#[inline(always)]
pub unsafe fn restore_interrupts(handle: usize) {
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
pub fn enable_irq_fiq() {
    unsafe {
        asm!(
        "msr daifclr, #0b11",
        options(nomem, nostack)
        )
    };
}