// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{ArgMatches, Command};
use thiserror::Error;

use crate::client::{self, Client};

pub fn command() -> Command {
    Command::new("list")
        .about("List packages")
        .long_about("List packages according to a filter")
        .subcommand_required(true)
        .subcommand(
            Command::new("installed")
                .about("List all installed packages")
                .visible_alias("li"),
        )
        .subcommand(
            Command::new("available")
                .about("List all available packages")
                .visible_alias("la"),
        )
}

/// Handle listing by filter
pub fn handle(args: &ArgMatches) -> Result<(), Error> {
    match args.subcommand() {
        Some(("available", _)) => {
            let _ = Client::system()?;
            Ok(())
        }
        Some(("installed", _)) => unimplemented!(),
        _ => unreachable!(),
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client error: {0}")]
    Client(#[from] client::Error),
}
