ENTRY(_start)

SECTIONS {
    . = 0x80000;

    .text : {
        KEEP(*(.text._start))
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    /* Stack top symbol (used in Rust asm) */
    . = ALIGN(16);
    . = . + 0x10000; /* 64 KiB stack */
    __stack_top = .;
    _end = .;
}