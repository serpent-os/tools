// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{Arg, ArgAction, Command};

mod install;
mod list;
mod remove;
mod version;

use crate::cli::version::*;

use self::{
    install::install_command,
    list::{list_command, list_command_handler},
    remove::remove_command,
};

/// Generate the CLI command structure
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
        .subcommand(list_command())
        .subcommand(install_command())
        .subcommand(remove_command())
        .subcommand(version_command())
}

/// Process all CLI arguments
pub fn process() {
    let matches = cli_main().get_matches();
    if matches.get_flag("version") {
        print_version();
        return;
    }
    match cli_main().get_matches().subcommand() {
        Some(("version", _)) => print_version(),
        Some(("list", a)) => list_command_handler(a),
        _ => unreachable!(),
    }
}
