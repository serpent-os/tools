// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::{ArgMatches, Command};
use itertools::Itertools;
use thiserror::Error;

use moss::{
    client::{self, Client},
    package::Flags,
};

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
    let root = args.get_one::<PathBuf>("root").unwrap().clone();

    let filter_flags = match args.subcommand() {
        Some(("available", _)) => Flags::AVAILABLE,
        Some(("installed", _)) => Flags::INSTALLED,
        _ => unreachable!(),
    };

    // Grab a client for the target, enumerate packages
    let mut client = Client::new_for_root(root)?;
    let pkgs = client.registry().list(filter_flags).collect_vec();
    if pkgs.is_empty() {
        return Err(Error::NoneFound);
    }

    // Print em
    for pkg in pkgs {
        println!(" - {:?}", pkg);
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client error: {0}")]
    Client(#[from] client::Error),

    #[error("no packages found")]
    NoneFound,
}
