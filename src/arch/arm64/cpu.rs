use core::arch::asm;

#[inline(always)]
pub fn core_id() -> usize {
    let mpidr: u64;
    unsafe { asm!("mrs {}, mpidr_el1", out(reg) mpidr) };
    (mpidr & 0xff) as usize
}
