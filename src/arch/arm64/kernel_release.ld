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
    __stack_top = . + 0x10000; /* 64 KiB stack */

    /* Discard debug sections */
    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.debug_*)
    }
    _end = .;
}