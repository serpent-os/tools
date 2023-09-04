// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{Arg, ArgAction, Command};

/// TODO: Add actual subcommands!
fn cli_main() -> Command {
    Command::new("moss")
        .about("Next generation package manager")
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .action(ArgAction::SetTrue),
        )
        .arg_required_else_help(true)
        .subcommand(Command::new("version").about("Display version and exit"))
}

/// Print program version
fn print_version() {
    println!("TODO: Set a version");
}

/// Main entry point
fn main() {
    let matches = cli_main().get_matches();
    if matches.get_flag("version") {
        print_version();
        return;
    }
    match cli_main().get_matches().subcommand() {
        Some(("version", _)) => print_version(),
        _ => unreachable!(),
    }
}
