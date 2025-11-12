use core::panic::PanicInfo;
use crate::kprintln;

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    kprintln!("\n\n\n\n\n\nPANIC OCCURRED:");
    if let Some(location) = _info.location() {
        kprintln!("  File: {}:{}:{}", location.file(), location.line(), location.column());
    } else {
        kprintln!("  No location information available.");
    }

    kprintln!("  Message: {}", _info.message());
    
    loop {}
}