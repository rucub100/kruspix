use core::arch::naked_asm;

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    naked_asm!(
        // check exception level
        "adr x6, .",
        "mrs x5, CurrentEL",
        "lsr x5, x5, #2",
        "cmp x5, #0",
        "b.eq 0f",
        "cmp x5, #1",
        "b.eq start_el1",
        "cmp x5, #2",
        "b.eq el2_to_el1",
        "cmp x5, #3",
        "b.eq el3_to_el2",
        "b 0f",
        // switch from EL3 to EL2
        "el3_to_el2:",
        "mrs x5, cpacr_el1",
        "orr x5, x5, #(0b11 << 20)",
        "msr  cpacr_el1, x5",
        "mov x5, (1 << 10) | (1 << 8) | (1 << 0)",
        "msr scr_el3, x5",
        "mov x5, (0b1111 << 6) | 9",
        "msr spsr_el3, x5",
        "adr x5, el2_to_el1",
        "msr elr_el3, x5",
        "eret",
        // switch from EL2 to EL1
        "el2_to_el1:",
        "mov x5, (1 << 31)",
        "msr hcr_el2, x5",
        "mov x5, (0b1111 << 6) | 5",
        "msr spsr_el2, x5",
        "msr elr_el2, x6",
        "eret",
        "start_el1:",
        // park secondary cores
        "bl _park_secondary_cores",
        // preserve DTB pointer from x0 to x20
        "mov x20, x0",
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
extern "C" fn _enable_early_mmu() {
    naked_asm!(
        "adr x6, mmu_enabled",
        "mov x7, #0xffff800000000000",
        "add x7, x7, x6",
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
        "mov x0, #0xff",
        "msr mair_el1, x0",
        "isb",
        // read system control register
        "mrs x0, sctlr_el1",
        // set the MMU enable bit
        "orr x0, x0, #1",
        // write back to system control register
        "msr sctlr_el1, x0",
        "isb",
        "br x7",
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