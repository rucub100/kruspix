// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ($crate::kernel::console::console_print(format_args!($($arg)*)));
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
