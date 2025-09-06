use core::arch::naked_asm;

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    naked_asm!(
        // park secondary cores
        "bl _park_secondary_cores",
        // preserve DTB pointer from x0 to x20
        "mov x20, x0",
        // set up the stack pointer
        "ldr x0, =__stack_top",
        "mov sp, x0",
        // clear frame pointer and link register
        "mov x29, xzr",
        "mov x30, xzr",
        // zero .bss section
        "bl _zero_bss",
        // restore DTB pointer to x0
        "mov x0, x20",
        // call the rust start_kernel function
        "bl start_kernel",
        // infinite Loop (in case start_kernel returns)
        "0:",
        "wfe",
        "b 0b",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn _park_secondary_cores() {
    naked_asm!(
        "mrs x3, mpidr_el1",
        "and x3, x3, #0xff",
        "cbz x3, 1f",
        "0:",
        "wfe",
        "b 0b",
        "1:",
        "ret",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn _zero_bss() {
    naked_asm!(
        "ldr x0, =__bss_start",
        "ldr x1, =__bss_end",
        "mov x2, xzr",
        "0:",
        "cmp x0, x1",
        "b.ge 1f",
        "str x2, [x0], #8",
        "b 0b",
        "1:",
        "ret",
    );
}