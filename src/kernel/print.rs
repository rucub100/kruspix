// TODO: replace the hardcoded backend with kernel abstraction layer
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ($crate::drivers::mini_uart::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! kprintln {
    () => ($crate::kprint!("[kruspix]\n"));
    ($fmt:expr, $($arg:tt)*) => (
        $crate::kprint!(concat!("[kruspix] ", $fmt, "\n"), $($arg)*)
    );
    ($msg:expr) => (
        $crate::kprint!(concat!("[kruspix] ", $msg, "\n"))
    );
}
