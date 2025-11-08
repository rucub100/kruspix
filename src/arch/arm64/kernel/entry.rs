use core::arch::naked_asm;

/// Entry point for the kernel.
/// This function is called by the bootloader or the firmware.
///
/// Starts primary core and parks secondary cores.
///
/// ### Important
/// Do not use or modify registers `x0` to `x3` in this function as they may contain
/// important boot information (e.g. DTB pointer in `x0`).
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    naked_asm!(
        "bl _park_secondary_cores",
        "b _start_primary",
    );
}

/// Park secondary cores in a low-power state.
///
/// ### Important
/// Do not use or modify registers `x0` to `x3` in this function as they may contain
/// important boot information (e.g. DTB pointer in `x0`).
#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn _park_secondary_cores() {
    naked_asm!(
        // Multiprocessor Affinity Register
        "mrs x4, mpidr_el1",
        // Affinity level 0
        "and x4, x4, #0xff",
        // continue only with the primary core (core 0) for now
        "cbz x4, 1f",
        "0:",
        "wfe",
        "b 0b",
        "1:",
        "ret",
    );
}

/// Entry point for the primary core.
///
/// This function checks the current exception level and branches to the appropriate
/// initialization function for that level.
///
/// ### Important
/// Do not use or modify registers `x0` to `x3` in this function as they may contain
/// important boot information (e.g. DTB pointer in `x0`).
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn _start_primary() -> ! {
    naked_asm!(
        // check current exception level
        "mrs x4, CurrentEL",
        "lsr x4, x4, #2",
        "cmp x4, #3",
        "b.eq _start_el3",
        "cmp x4, #2",
        "b.eq _start_el2",
        "cmp x4, #1",
        "b.eq _start_el1",
        // unsupported exception level
        "0:",
        "wfe",
        "b 0b",
    );
}

/// Initialization and configuration for EL3 (Secure Monitor).
///
/// ### Important
/// Do not use or modify registers `x0` to `x3` in this function as they may contain
/// important boot information (e.g. DTB pointer in `x0`).
#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn _start_el3() {
    naked_asm!(
        // System Control
        "mrs x4, sctlr_el3",
        // disable MMU
        "bic x4, x4, #(1 << 0)",
        // enable alignment check
        "orr x4, x4, #(1 << 1)",
        // disable data cache
        "bic x4, x4, #(1 << 2)",
        // enable stack alignment check
        "orr x4, x4, #(1 << 3)",
        // disable instruction cache
        "bic x4, x4, #(1 << 12)",
        // enable WXN protection
        "orr x4, x4, #(1 << 19)",
        // set exception endianness to little endian
        "bic x4, x4, #(1 << 25)",
        // set SCTLR_EL3
        "msr sctlr_el3, x4",
        // Secure Configuration
        // use AArch64 for next lower exception levels (bit 10)
        // enable HVC at EL3/2/1 (bit 8)
        // set non-secure state for lower exception levels (bit 0)
        "mov x4, (1 << 10) | (1 << 8) | (1 << 0)",
        "msr scr_el3, x4",
        // Hypervisor Configuration - set execution state for EL1 to AArch64
        "mov x4, (1 << 31)",
        "msr hcr_el2, x4",
        // Architectural Feature Trap - don't trap any
        "mov x4, xzr",
        "msr cptr_el3, x4",
        "msr cptr_el2, x4",
        // Program Status - mask exceptions and set EL1h
        "mov x4, (0b1111 << 6) | 0b0101",
        "msr spsr_el3, x4",
        // Exception Link
        "adr x4, _start_primary",
        "msr elr_el3, x4",
        "eret",
    );
}

/// Initialization and configuration for EL2 (Hypervisor).
///
/// ### Important
/// Do not use or modify registers `x0` to `x3` in this function as they may contain
/// important boot information (e.g. DTB pointer in `x0`).
#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn _start_el2() {
    naked_asm!(
        // System Control
        "mrs x4, sctlr_el2",
        // disable MMU
        "bic x4, x4, #(1 << 0)",
        // enable alignment check
        "orr x4, x4, #(1 << 1)",
        // disable data cache
        "bic x4, x4, #(1 << 2)",
        // enable stack alignment check
        "orr x4, x4, #(1 << 3)",
        // disable instruction cache
        "bic x4, x4, #(1 << 12)",
        // enable WXN protection
        "orr x4, x4, #(1 << 19)",
        // set exception endianness to little endian
        "bic x4, x4, #(1 << 25)",
        // set SCTLR_EL2
        "msr sctlr_el2, x4",
        // Hypervisor Configuration - set execution state for EL1 to AArch64
        "mov x4, (1 << 31)",
        "msr hcr_el2, x4",
        // Architectural Feature Trap - don't trap any
        "mov x4, xzr",
        "msr cptr_el2, x4",
        // Program Status - mask exceptions and set EL1h
        "mov x4, (0b1111 << 6) | 0b0101",
        "msr spsr_el2, x4",
        // Exception Link
        "adr x5, _start_primary",
        "msr elr_el2, x5",
        "eret",
    );
}

/// Initialization and configuration for EL1 (Kernel).
///
/// ### Important
/// Do not use or modify registers `x0` to `x3` in this function as they may contain
/// important boot information (e.g. DTB pointer in `x0`).
#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn _start_el1() {
    naked_asm!(
        // preserve DTB pointer from x0 to x20
        "mov x20, x0",
        // System Control
        "mrs x4, sctlr_el1",
        // disable MMU
        "bic x4, x4, #(1 << 0)",
        // enable alignment check
        "orr x4, x4, #(1 << 1)",
        // disable data cache
        "bic x4, x4, #(1 << 2)",
        // enable stack alignment check
        "orr x4, x4, #(1 << 3)",
        "orr x4, x4, #(1 << 4)",
        // disable WXN protection
        "bic x4, x4, #(1 << 19)",
        // set explicit data access at EL0 to little-endian
        "bic x4, x4, #(1 << 24)",
        // set exception endianness to little-endian
        "bic x4, x4, #(1 << 25)",
        // disable cache maintenance instructions in EL0
        "bic x4, x4, #(1 << 26)",
        "msr sctlr_el1, x4",
        // Coprocessor Access Control
        "mrs x4, cpacr_el1",
        "orr x4, x4, #(0b11 << 20)",
        "msr  cpacr_el1, x4",
        // enable early MMU
        "bl _enable_early_mmu",
        "mmu_enabled:",
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
        ".balign 16",
        ".space 0x10000",
        "__stack_top:",
    );
}

/// Enable the MMU with a simple identity-mapped page table.
///
/// ### Important
/// Do not use or modify register `x20` in this function as it is used to preserve
/// the DTB pointer across calls.
#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn _enable_early_mmu() {
    naked_asm!(
        // setup virtual return address
        "mov x7, #0xffff800000000000",
        "add x30, x30, x7",
        "mrs x2, tcr_el1",
        // set T0SZ and T1SZ to 16 (48-bit VA)
        "movz x3, #16",
        "bfi x2, x3, #0, #6",
        "bfi x2, x3, #16, #6",
        // set TG0 and TG1 to 4KB granule
        "bfi x2, xzr, #14, #2",
        "bfi x2, xzr, #30, #2",
        // configure page tables
        "adr x0, LEVEL_0_TABLE_DESCRIPTOR_0",
        "msr ttbr0_el1, x0",
        "adr x3, LEVEL_1_BLOCK_DESCRIPTOR_0",
        "mov x4, #0x701",
        "str x4, [x3]",
        "orr x3, x3, #0b11",
        "str x3, [x0]",
        "adr x1, LEVEL_0_TABLE_DESCRIPTOR_1",
        "msr ttbr1_el1, x1",
        "add x1, x1, #0x800",
        "adr x3, LEVEL_1_BLOCK_DESCRIPTOR_1",
        "mov x4, #0x701",
        "str x4, [x3]",
        "orr x3, x3, #0b11",
        "str x3, [x1]",
        "msr tcr_el1, x2",
        // Device-nGnRnE memory for all memory
        "mov x0, #0x00",
        "msr mair_el1, x0",
        "isb",
        // read system control register
        "mrs x0, sctlr_el1",
        // set the MMU enable bit
        "orr x0, x0, #1",
        // write back to system control register
        "msr sctlr_el1, x0",
        "isb",
        "ret",
        // DATA --------------------------------------------------------------------------------
        // setup early page tables
        ".balign 4096",
        "LEVEL_0_TABLE_DESCRIPTOR_0:",
        ".rept 512",
        ".quad 0",
        ".endr",
        "LEVEL_0_TABLE_DESCRIPTOR_1:",
        ".rept 512",
        ".quad 0",
        ".endr",
        "LEVEL_1_BLOCK_DESCRIPTOR_0:",
        ".rept 512",
        ".quad 0x701",
        ".endr",
        "LEVEL_1_BLOCK_DESCRIPTOR_1:",
        ".rept 512",
        ".quad 0x701",
        ".endr",
    );
}

/// Zero out the .bss section.
///
/// ### Important
/// Do not use or modify register `x20` in this function as it is used to preserve
/// the DTB pointer across calls.
#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn _zero_bss() {
    naked_asm!(
        "ldr x4, =__bss_start",
        "ldr x5, =__bss_end",
        "mov x6, xzr",
        "0:",
        "cmp x4, x5",
        "b.ge 1f",
        "str x6, [x4], #8",
        "b 0b",
        "1:",
        "ret",
    );
}