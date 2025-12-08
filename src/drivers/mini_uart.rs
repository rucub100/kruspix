//! Mini UART output-only debug driver for Raspberry Pi 3b.
//! This is only a minimal and incomplete implementation.
//! It's just a hack to get some debug output during early boot.
//! Don't use this for anything serious!

use core::fmt::{self, Write};
use core::ptr::{read_volatile, write_volatile};

const AUX_BASE: usize = 0x3F215000;
// Mini Uart I/O Data
const AUX_MU_IO_REG: *mut u8 = (AUX_BASE + 0x40) as *mut u8;
// Mini Uart Line Status
const AUX_MU_LSR_REG: *mut u8 = (AUX_BASE + 0x54) as *mut u8;

const TX_EMPTY: u8 = 1 << 5;

pub struct MiniUart;

impl MiniUart {
    pub fn write_byte(&self, byte: u8) {
        while unsafe { read_volatile(AUX_MU_LSR_REG) } & TX_EMPTY == 0 {
            core::hint::spin_loop();
        }

        unsafe {
            write_volatile(AUX_MU_IO_REG, byte);
        }
    }
}

impl Write for MiniUart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}

static mut GLOBAL_UART: MiniUart = MiniUart;

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;

    // assuming single-threaded early kernel context
    unsafe {
        #[allow(static_mut_refs)]
        GLOBAL_UART.write_fmt(args).unwrap();
    }
}
