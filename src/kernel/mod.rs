use crate::mm::layout::LINEAR_MAP_OFFSET;

pub mod boot;



#[inline]
pub fn virt_to_phys(va: usize) -> usize {
    if va >= LINEAR_MAP_OFFSET {
        va - LINEAR_MAP_OFFSET
    } else {
        va
    }
}

pub fn kernel_addr_size() -> (usize, usize) {
    let kernel_start: usize;
    let kernel_end: usize;

    unsafe {
        core::arch::asm!("
            ldr {}, =_start
            ldr {}, =_end
        ", out(reg) kernel_start, out(reg) kernel_end);
    }

    (virt_to_phys(kernel_start), kernel_end - kernel_start)
}

pub fn kernel_bss_size() -> usize {
    let bss_start: usize;
    let bss_end: usize;

    unsafe {
        core::arch::asm!("
            ldr {}, =__bss_start
            ldr {}, =__bss_end
        ", out(reg) bss_start, out(reg) bss_end);
    }

    bss_end - bss_start
}
