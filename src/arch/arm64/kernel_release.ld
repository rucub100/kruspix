ENTRY(_start)

__vma_base = 0xFFFF800000080000;
__lma_base = 0x80000;
__lma_offset = __lma_base - __vma_base;

SECTIONS {
    . = __vma_base;

    .text : AT(. + __lma_offset) {
        KEEP(*(.text._start))
        *(.text .text.*)
    }

    .rodata : AT(. + __lma_offset) {
        *(.rodata .rodata.*)
    }

    .data : AT(. + __lma_offset) {
        *(.data .data.*)
    }

    .bss : AT(. + __lma_offset) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    /* Discard debug sections */
    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.debug_*)
    }

    _end = .;
}