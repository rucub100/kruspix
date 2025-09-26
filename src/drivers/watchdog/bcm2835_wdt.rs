use core::ptr::write_volatile;

const PM_BASE: *mut u32 = 0x3F100000 as *mut u32;
const PM_PASSWORD: u32 = 0x5a000000;

const PM_RSTC: usize = 0x1c;
const PM_RSTC_RESET: u32 = 0x00000102;

pub fn bcm2835_wdt_disable() {
    unsafe {
        write_volatile(PM_BASE.add(PM_RSTC), PM_PASSWORD | PM_RSTC_RESET);
    }
}
