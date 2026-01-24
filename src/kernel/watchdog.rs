// SPDX-License-Identifier: MIT
// Copyright (c) 2025-2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::time::Duration;

use crate::drivers::Device;
use crate::kernel::shell;
use crate::kernel::shell::ShellCommand;
use crate::kernel::sync::SpinLock;
use crate::kernel::terminal::get_system_terminal;

pub trait Watchdog: Device {
    fn is_running(&self) -> bool;
    fn start(&self);
    fn stop(&self);
    fn get_default_timeout(&self) -> Duration;
    fn get_max_timeout(&self) -> Duration;
    fn get_min_timeout(&self) -> Duration;
    fn set_timeout(&self, timeout: Duration);
    fn get_countdown(&self) -> Duration;
    fn acknowledge(&self);
}

static WATCHDOGS: SpinLock<Vec<Arc<dyn Watchdog>>> = SpinLock::new(Vec::new());

pub fn register_watchdog(wdt: Arc<dyn Watchdog>) {
    let mut watchdogs = WATCHDOGS.lock();
    watchdogs.push(wdt);
}

pub(super) fn init() -> Result<(), ()> {
    shell::register_command(ShellCommand::new(
        "watchdog",
        "Watchdog control",
        |_, args| {
            let print_usage = || {
                if let Some(terminal) = get_system_terminal() {
                    terminal
                        .write(b"Usage: watchdog <status|start|stop|set_timeout <seconds>|ack>\n");
                }
            };

            let status = || {
                let watchdog = WATCHDOGS.lock();
                let watchdog = watchdog.first().unwrap();

                let is_running = watchdog.is_running();
                let default_timeout = watchdog.get_default_timeout();
                let min_timeout = watchdog.get_min_timeout();
                let max_timeout = watchdog.get_max_timeout();
                let countdown = watchdog.get_countdown();

                if let Some(terminal) = get_system_terminal() {
                    terminal.write(b"Watchdog Status: ");
                    terminal.write(if is_running { b"RUNNING" } else { b"STOPPED" });
                    terminal.write(b"\n");
                    terminal.write(b"Timeout Settings:\n");
                    terminal.write(b"  Default: ");
                    terminal.write(default_timeout.as_secs().to_string().as_bytes());
                    terminal.write(b" seconds\n");
                    terminal.write(b"  Min: ");
                    terminal.write(min_timeout.as_secs().to_string().as_bytes());
                    terminal.write(b" seconds\n");
                    terminal.write(b"  Max: ");
                    terminal.write(max_timeout.as_secs().to_string().as_bytes());
                    terminal.write(b" seconds\n");

                    if is_running {
                        terminal.write(b"Current Countdown: ");
                        terminal.write(countdown.as_secs().to_string().as_bytes());
                        terminal.write(b" seconds\n");
                    }
                }
            };

            let start = || {
                let watchdog = WATCHDOGS.lock();
                watchdog.first().unwrap().start();
            };

            let stop = || {
                let watchdog = WATCHDOGS.lock();
                watchdog.first().unwrap().stop();
            };

            let set_timeout = |secs: u64| {
                let watchdog = WATCHDOGS.lock();
                let watchdog = watchdog.first().unwrap();

                let min_timeout = watchdog.get_min_timeout().as_secs();
                let max_timeout = watchdog.get_max_timeout().as_secs();

                if secs < min_timeout || secs > max_timeout {
                    if let Some(terminal) = get_system_terminal() {
                        terminal.write(b"Usage: Timeout must be between ");
                        terminal.write(min_timeout.to_string().as_bytes());
                        terminal.write(b" and ");
                        terminal.write(max_timeout.to_string().as_bytes());
                        terminal.write(b" seconds.\n");
                    }
                    return;
                }

                watchdog.set_timeout(Duration::from_secs(secs));
            };

            let ack = || {
                let watchdog = WATCHDOGS.lock();
                watchdog.first().unwrap().acknowledge();
            };

            if args.is_empty() {
                print_usage();
                return;
            }

            match args[0] {
                "status" => status(),
                "start" => start(),
                "stop" => stop(),
                "set_timeout" => {
                    if args.len() != 2 {
                        print_usage();
                        return;
                    }
                    if let Ok(secs) = args[1].parse::<u64>() {
                        set_timeout(secs);
                    } else {
                        print_usage();
                    }
                }
                "ack" => ack(),
                _ => print_usage(),
            }
        },
    ));

    Ok(())
}
