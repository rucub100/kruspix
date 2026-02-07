// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

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
    naked_asm!("bl _park_secondary_cores", "b _start_primary");
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
        // FIXME: we assume boot core has id 0 but we also could read it from FDT (if available)
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
extern "C" fn _start_primary() -> ! {
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
        // TODO: CNTKCTL_EL2 - access control for timers
        // TODO: CNTVOFF_EL2 - make sure offset is zero
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
        // JTAG debugging trap
        // "jtag:",
        // "wfe",
        // "b jtag",
        // preserve DTB pointer from x0 to x20
        "mov x20, x0",
        // set up early exception vector table
        "adr x0, EARLY_VECTOR_TABLE",
        "msr vbar_el1, x0",
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
        // TODO: CNTKCTL_EL1 - access control for timers
        // enable early MMU
        "bl _enable_early_mmu",
        "mmu_enabled:",
        // update early exception vector table to it's virtual address
        "adr x0, EARLY_VECTOR_TABLE",
        "msr vbar_el1, x0",
        // set up the stack pointer
        "adr x0, __stack_top",
        "mov sp, x0",
        // clear frame pointer and link register
        "mov x29, xzr",
        "mov x30, xzr",
        // zero .bss section
        "bl _zero_bss",
        // restore DTB pointer to x0
        "adr x0, __fdt_address",
        "str x20, [x0]",
        // call the rust start_kernel function
        "bl start_kernel",
        // infinite Loop (in case start_kernel returns)
        "0:",
        "wfe",
        "b 0b",
        // DATA --------------------------------------------------------------------------------
        // variable to hold the DTB address
        ".balign 8",
        "__fdt_address:",
        ".global __fdt_address",
        ".quad 0",
        // setup early kernel stack (64KB)
        ".balign 16",
        ".space 0x10000",
        "__stack_top:",
        // setup early exception vector table
        ".balign 2048",
        "EARLY_VECTOR_TABLE:",
        // EL1t
        // - Synchronous
        ".balign 128",
        "b 1f",
        ".quad 1",
        "1:",
        "wfe",
        "b 1b",
        // - IRQ
        ".balign 128",
        "b 2f",
        ".quad 2",
        "2:",
        "wfe",
        "b 2b",
        // - FIQ
        ".balign 128",
        "b 3f",
        ".quad 3",
        "3:",
        "wfe",
        "b 3b",
        // - SError
        ".balign 128",
        "b 4f",
        ".quad 4",
        "4:",
        "wfe",
        "b 4b",
        // EL1h
        // - Synchronous
        ".balign 128",
        "b 5f",
        ".quad 5",
        "5:",
        "wfe",
        "b 5b",
        // - IRQ
        ".balign 128",
        "b _el1h_irq_handler",
        // - FIQ
        ".balign 128",
        "b 7f",
        ".quad 7",
        "7:",
        "wfe",
        "b 7b",
        // - SError
        ".balign 128",
        "b 8f",
        ".quad 8",
        "8:",
        "wfe",
        "b 8b",
        // EL0 using Aarch64
        // - Synchronous
        ".balign 128",
        "b 9f",
        ".quad 9",
        "9:",
        "wfe",
        "b 9b",
        // - IRQ
        ".balign 128",
        "b 10f",
        ".quad 10",
        "10:",
        "wfe",
        "b 10b",
        // - FIQ
        ".balign 128",
        "b 11f",
        ".quad 11",
        "11:",
        "wfe",
        "b 11b",
        // - SError
        ".balign 128",
        "b 12f",
        ".quad 12",
        "12:",
        "wfe",
        "b 12b",
        // EL0 using Aarch32
        // - Synchronous
        ".balign 128",
        "b 13f",
        ".quad 13",
        "13:",
        "wfe",
        "b 13b",
        // - IRQ
        ".balign 128",
        "b 14f",
        ".quad 14",
        "14:",
        "wfe",
        "b 14b",
        // - FIQ
        ".balign 128",
        "b 15f",
        ".quad 15",
        "15:",
        "wfe",
        "b 15b",
        // - SError
        ".balign 128",
        "b 16f",
        ".quad 16",
        "16:",
        "wfe",
        "b 16b",
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
        // setup translation control
        "ldr x2, =0xb5103510",
        "msr tcr_el1, x2",
        // configure page tables
        "adr x0, LEVEL_0_TABLE_DESCRIPTOR_0",
        "msr ttbr0_el1, x0",
        "adr x3, LEVEL_1_BLOCK_DESCRIPTOR_0",
        "orr x3, x3, #0b11",
        "str x3, [x0]",
        "adr x1, LEVEL_0_TABLE_DESCRIPTOR_1",
        "msr ttbr1_el1, x1",
        "add x1, x1, #0x800",
        "adr x3, LEVEL_1_BLOCK_DESCRIPTOR_1",
        "orr x3, x3, #0b11",
        "str x3, [x1]",
        // Memory Attribute Indirection
        // -> Attr0 - Device-nGnRnE
        // -> Attr1 - Normal memory, non-cacheable
        // -> Attr2 - Normal memory, write-back non-transient read/write allocate
        "ldr x0, =0xff4400",
        "msr mair_el1, x0",
        "isb",
        // read system control register
        "mrs x0, sctlr_el1",
        // enable data cache
        "orr x0, x0, #(1 << 2)",
        // enable instruction cache
        "orr x0, x0, #(1 << 12)",
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
        ".set addr, 0",
        ".rept 512",
        // AF = 1; SH = inner sharable; AttrIndx = 0
        ".quad addr | 0x701",
        ".set addr, addr + 0x40000000",
        ".endr",
        "LEVEL_1_BLOCK_DESCRIPTOR_1:",
        ".set addr, 0",
        ".rept 512",
        // AF = 1; SH = inner sharable; AttrIndx = 2
        ".quad addr | 0x709",
        ".set addr, addr + 0x40000000",
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

#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn _el1h_irq_handler() {
    naked_asm!(
        // save registers
        "sub sp, sp, #16 * 17",
        "stp x0, x1, [sp, #16 * 0]",
        "stp x2, x3, [sp, #16 * 1]",
        "stp x4, x5, [sp, #16 * 2]",
        "stp x6, x7, [sp, #16 * 3]",
        "stp x8, x9, [sp, #16 * 4]",
        "stp x10, x11, [sp, #16 * 5]",
        "stp x12, x13, [sp, #16 * 6]",
        "stp x14, x15, [sp, #16 * 7]",
        "stp x16, x17, [sp, #16 * 8]",
        "stp x18, x19, [sp, #16 * 9]",
        "stp x20, x21, [sp, #16 * 10]",
        "stp x22, x23, [sp, #16 * 11]",
        "stp x24, x25, [sp, #16 * 12]",
        "stp x26, x27, [sp, #16 * 13]",
        "stp x28, x29, [sp, #16 * 14]",
        "mrs x9, elr_el1",
        "mrs x10, spsr_el1",
        "stp x30, x9, [sp, #16 * 15]",
        "str x10, [sp, #16 * 16]",
        // call the global IRQ dispatcher
        "bl global_irq_dispatch",
        // restore registers
        "ldp x30, x9, [sp, #16 * 15]",
        "ldr x10, [sp, #16 * 16]",
        "msr elr_el1, x9",
        "msr spsr_el1, x10",
        "ldp x0, x1, [sp, #16 * 0]",
        "ldp x2, x3, [sp, #16 * 1]",
        "ldp x4, x5, [sp, #16 * 2]",
        "ldp x6, x7, [sp, #16 * 3]",
        "ldp x8, x9, [sp, #16 * 4]",
        "ldp x10, x11, [sp, #16 * 5]",
        "ldp x12, x13, [sp, #16 * 6]",
        "ldp x14, x15, [sp, #16 * 7]",
        "ldp x16, x17, [sp, #16 * 8]",
        "ldp x18, x19, [sp, #16 * 9]",
        "ldp x20, x21, [sp, #16 * 10]",
        "ldp x22, x23, [sp, #16 * 11]",
        "ldp x24, x25, [sp, #16 * 12]",
        "ldp x26, x27, [sp, #16 * 13]",
        "ldp x28, x29, [sp, #16 * 14]",
        "add sp, sp, #16 * 17",
        "eret",
    );
}
