// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! # Kernel Shell
//!
//! The `shell` module provides a minimalist, reactive command-line interface for the
//! kruspix kernel. It is designed to function as the primary system interface during
//! early boot or emergency recovery, before a full userspace `init` process is available.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::str::from_utf8;

use crate::kernel::power::{system_power_off, system_restart};
use crate::kernel::sync::SpinLock;
use crate::kernel::terminal::{LineListener, get_system_terminal};

pub type CommandHandler = fn(&KernelShell, &[&str]);

pub struct ShellCommand {
    name: &'static str,
    description: &'static str,
    handler: CommandHandler,
}

impl ShellCommand {
    pub const fn new(
        name: &'static str,
        description: &'static str,
        handler: CommandHandler,
    ) -> Self {
        ShellCommand {
            name,
            description,
            handler,
        }
    }
}

const COMMAND_REGISTRY: &[ShellCommand] = &[
    ShellCommand {
        name: "help",
        description: "Display this help message",
        handler: KernelShell::cmd_help,
    },
    ShellCommand {
        name: "echo",
        description: "Echo the input arguments",
        handler: KernelShell::cmd_echo,
    },
    ShellCommand {
        name: "reboot",
        description: "Reboot the system",
        handler: KernelShell::cmd_reboot,
    },
    ShellCommand {
        name: "poweroff",
        description: "Power off the system",
        handler: KernelShell::cmd_poweroff,
    },
    ShellCommand {
        name: "clear",
        description: "Clear the terminal screen",
        handler: KernelShell::cmd_clear,
    },
];

static DYNAMIC_COMMAND_REGISTRY: SpinLock<Vec<ShellCommand>> = SpinLock::new(Vec::new());

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
            let welcome_message = "\n\nWelcome to the kruspix kernel shell!\n";
            let help_hint = "Type 'help' for a list of commands.\n";
            terminal.write(welcome_message.as_bytes());
            terminal.write(help_hint.as_bytes());
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

        if let Some(cmd) = COMMAND_REGISTRY.iter().find(|c| c.name == command) {
            (cmd.handler)(self, &args);
        } else if let Some(cmd) = DYNAMIC_COMMAND_REGISTRY
            .lock()
            .iter()
            .find(|c| c.name == command)
        {
            (cmd.handler)(self, &args);
        } else {
            if let Some(terminal) = get_system_terminal() {
                terminal.write(b"Unknown command: ");
                terminal.write(command.as_bytes());
                terminal.write(b"\nType 'help -a' for a full list.\n");
            }
        }
    }

    fn cmd_help(&self, args: &[&str]) {
        if let Some(terminal) = get_system_terminal() {
            let show_all = args.contains(&"-a");

            terminal.write(b"Kruspix kernel shell commands:\n");
            terminal.write(b"------------------------------\n");

            for cmd in COMMAND_REGISTRY {
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

            if show_all {
                let dynamic_registry = DYNAMIC_COMMAND_REGISTRY.lock();
                for cmd in dynamic_registry.iter() {
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
            }

            terminal.write(b"------------------------------\n");

            if !show_all {
                terminal.write(b"\nType 'help -a' to see all commands.\n");
            }
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

    fn cmd_clear(&self, _args: &[&str]) {
        if let Some(terminal) = get_system_terminal() {
            // ANSI escape code to clear the screen and move the cursor to the top-left
            terminal.write(b"\x1B[2J\x1B[H");
        }
    }

    fn cmd_reboot(&self, _args: &[&str]) {
        system_restart();
    }

    fn cmd_poweroff(&self, _args: &[&str]) {
        system_power_off();
    }
}

pub fn register_command(cmd: ShellCommand) {
    let mut registry = DYNAMIC_COMMAND_REGISTRY.lock();
    registry.push(cmd);
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
