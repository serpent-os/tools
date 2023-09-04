// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{ArgMatches, Command};

pub fn list_command() -> Command {
    Command::new("list")
        .about("List packages")
        .long_about("List packages according to a filter")
        .subcommand_required(true)
        .subcommand(Command::new("installed").about("List all installed packages"))
        .subcommand(Command::new("available").about("List all available packages"))
}

/// Handle listing by filter
pub fn list_command_handler(_: &ArgMatches) {
    println!("Listage");
}
