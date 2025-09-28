use core::ptr::{read_volatile, write_volatile};

const PM_BASE: *mut u32 = 0x3F100000 as *mut u32;
const PM_PASSWORD: u32 = 0x5a000000;

const PM_RSTC: usize = 0x1c;
const PM_WDOG: usize = 0x24;

const PM_RSTC_WRCFG_FULL_RESET: u32 = 0x00000020;

pub fn bcm2835_wdt_disable() {
    unsafe {
        let rstc = read_volatile(PM_BASE.byte_add(PM_RSTC));
        write_volatile(PM_BASE.byte_add(PM_RSTC), PM_PASSWORD | (rstc & !PM_RSTC_WRCFG_FULL_RESET));
        write_volatile(PM_BASE.byte_add(PM_WDOG), PM_PASSWORD | 0);
    }
}
