// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{Arg, ArgAction, Command};

mod install;
mod list;
mod remove;
mod version;

/// Generate the CLI command structure
fn command() -> Command {
    Command::new("moss")
        .about("Next generation package manager")
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .action(ArgAction::SetTrue),
        )
        .arg_required_else_help(true)
        .subcommand(list::command())
        .subcommand(install::command())
        .subcommand(remove::command())
        .subcommand(version::command())
}

/// Process all CLI arguments
pub fn process() {
    let matches = command().get_matches();
    if matches.get_flag("version") {
        version::print();
        return;
    }
    match command().get_matches().subcommand() {
        Some(("version", _)) => version::print(),
        Some(("list", a)) => list::handle(a),
        _ => unreachable!(),
    }
}
