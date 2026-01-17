//! # Kernel Shell
//!
//! The `shell` module provides a minimalist, reactive command-line interface for the
//! kruspix kernel. It is designed to function as the primary system interface during
//! early boot or emergency recovery, before a full userspace `init` process is available.

use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::str::from_utf8;

use crate::kernel::power::{system_power_off, system_restart};
use crate::kernel::terminal::{LineListener, get_system_terminal};

struct CommandInfo {
    name: &'static str,
    description: &'static str,
}

const COMMAND_LIST: &[CommandInfo] = &[
    CommandInfo {
        name: "help",
        description: "Display this list of supported commands",
    },
    CommandInfo {
        name: "echo",
        description: "Print the provided arguments to the terminal",
    },
    CommandInfo {
        name: "reboot",
        description: "Perform a soft reset of the system",
    },
    CommandInfo {
        name: "poweroff",
        description: "Shut down the system safely",
    },
    CommandInfo {
        name: "clear",
        description: "Clear the terminal screen using ANSI escape codes",
    },
];

pub struct KernelShell;

impl KernelShell {
    pub const fn new() -> Self {
        KernelShell
    }

    pub fn start(self) {
        self.print_welcome_message();
        self.print_prompt();

        if let Some(terminal) = get_system_terminal() {
            terminal.add_listener(Arc::new(self));
        }
    }

    fn print_welcome_message(&self) {
        if let Some(terminal) = get_system_terminal() {
            let welcome_message =
                "\n\nWelcome to the kruspix kernel shell!\nType 'help' for a list of commands.\n";
            terminal.write(welcome_message.as_bytes());
        }
    }

    fn print_prompt(&self) {
        if let Some(terminal) = get_system_terminal() {
            terminal.write("\nkruspix> ".as_bytes());
        }
    }

    fn exec(&self, line: &str) {
        let mut parts = line.split_whitespace();
        let command = match parts.next() {
            Some(cmd) => cmd,
            None => return,
        };
        let args: Vec<&str> = parts.collect();

        match command {
            "help" => self.cmd_help(),
            "echo" => self.cmd_echo(&args),
            "reboot" => self.cmd_reboot(),
            "poweroff" => self.cmd_poweroff(),
            "clear" => self.cmd_clear(),
            _ => {
                if let Some(terminal) = get_system_terminal() {
                    let msg = format!("Unknown command: {}\n", command);
                    terminal.write(msg.as_bytes());
                }
            }
        }
    }

    fn cmd_help(&self) {
        if let Some(terminal) = get_system_terminal() {
            terminal.write(b"Kruspix kernel shell - available commands:\n");
            terminal.write(b"------------------------------------------\n");

            for cmd in COMMAND_LIST {
                terminal.write(b"  ");
                terminal.write(cmd.name.as_bytes());

                let padding = 10usize.saturating_sub(cmd.name.len());
                for _ in 0..padding {
                    terminal.write(b" ");
                }

                terminal.write(b" - ");
                terminal.write(cmd.description.as_bytes());
                terminal.write(b"\n");
            }
            terminal.write(b"------------------------------------------\n");
        }
    }

    fn cmd_echo(&self, args: &[&str]) {
        if let Some(terminal) = get_system_terminal() {
            for (i, arg) in args.iter().enumerate() {
                terminal.write(arg.as_bytes());
                if i < args.len() - 1 {
                    terminal.write(b" ");
                }
            }
            terminal.write(b"\n");
        }
    }

    fn cmd_clear(&self) {
        if let Some(terminal) = get_system_terminal() {
            // ANSI escape code to clear the screen and move the cursor to the top-left
            terminal.write(b"\x1B[2J\x1B[H");
        }
    }

    fn cmd_reboot(&self) {
        system_restart();
    }

    fn cmd_poweroff(&self) {
        system_power_off();
    }
}

impl LineListener for KernelShell {
    fn on_line(&self, line: &[u8]) {
        if let Some(terminal) = get_system_terminal() {
            let line = from_utf8(line);

            match line {
                Ok(cmd) => {
                    self.exec(cmd);
                }
                Err(_) => {
                    terminal.write(b"Error: Invalid UTF-8 input");
                }
            }

            self.print_prompt();
        }
    }
}
