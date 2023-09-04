// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::Command;

/// TODO: Add actual subcommands!
fn cli_main() -> Command {
    Command::new("moss").about("Next generation package manager")
}

/// Main entry point
fn main() {
    match cli_main().get_matches().subcommand() {
        _ => println!("We should implement a real CLI, huh?"),
    }
}
