// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{Arg, ArgAction, Command};
use thiserror::Error;

mod extract;
mod info;
mod inspect;
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
        .arg(
            Arg::new("root")
                .short('D')
                .long("directory")
                .global(true)
                .help("Root directory")
                .action(ArgAction::Set)
                .default_value("/"),
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes-all")
                .global(true)
                .help("Assume yes for all questions")
                .action(ArgAction::SetTrue),
        )
        .arg_required_else_help(true)
        .subcommand(extract::command())
        .subcommand(info::command())
        .subcommand(inspect::command())
        .subcommand(install::command())
        .subcommand(list::command())
        .subcommand(remove::command())
        .subcommand(version::command())
}

/// Process all CLI arguments
pub fn process() -> Result<(), Error> {
    let matches = command().get_matches();
    if matches.get_flag("version") {
        version::print();
        return Ok(());
    }
    match command().get_matches().subcommand() {
        Some(("extract", args)) => extract::handle(args).map_err(Error::Extract),
        Some(("inspect", args)) => inspect::handle(args).map_err(Error::Inspect),
        Some(("version", _)) => {
            version::print();
            Ok(())
        }
        Some(("list", a)) => list::handle(a).map_err(Error::List),
        _ => unreachable!(),
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("error handling list: {0}")]
    List(#[from] list::Error),

    #[error("error handling inspect: {0}")]
    Inspect(#[from] inspect::Error),

    #[error("error in extraction: {0}")]
    Extract(#[from] extract::Error),
}
