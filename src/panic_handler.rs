use core::panic::PanicInfo;
use crate::kprintln;

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    kprintln!("\n\n\n\n\n\n[kruspix] PANIC OCCURRED:");
    if let Some(location) = _info.location() {
        kprintln!("[kruspix]   File: {}:{}:{}", location.file(), location.line(), location.column());
    } else {
        kprintln!("[kruspix]   No location information available.");
    }

    kprintln!("[kruspix]   Message: {}", _info.message());
    
    loop {}
}