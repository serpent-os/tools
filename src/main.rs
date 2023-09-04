// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::Command;

/// TODO: Add actual subcommands!
fn cli_main() -> Command {
    Command::new("moss")
        .about("Next generation package manager")
        .arg_required_else_help(true)
        .subcommand_required(true)
        .subcommand(Command::new("version").about("Display version and exit"))
}

/// Main entry point
fn main() {
    match cli_main().get_matches().subcommand() {
        Some(("version", _)) => println!("Gimme a break I only just started!"),
        _ => println!("We should implement a real CLI, huh?"),
    }
}
