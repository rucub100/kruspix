use crate::mm::virt_to_phys;

pub mod devicetree;
pub mod print;
pub mod sync;
pub mod console;

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
