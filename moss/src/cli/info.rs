// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::{arg, ArgMatches, Command};
use futures::StreamExt;
use moss::{
    client::{self, Client},
    package::{Flags, Name},
};
use thiserror::Error;

pub fn command() -> Command {
    Command::new("info")
        .about("Query packages")
        .long_about("List detailed package information from all available sources")
        .arg(arg!(<NAME> ... "packages to query").value_parser(clap::value_parser!(String)))
}

/// For all arguments, try to match a package
pub async fn handle(args: &ArgMatches) -> Result<(), Error> {
    let pkgs = args
        .get_many::<String>("NAME")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    let root = args.get_one::<PathBuf>("root").unwrap().clone();
    let client = Client::new_for_root(root).await?;

    for pkg in pkgs {
        let nom = Name::from(pkg.clone());
        let resolved = client
            .registry
            .by_name(&nom, Flags::AVAILABLE)
            .collect::<Vec<_>>()
            .await;
        if resolved.len() == 0 {
            return Err(Error::NotFound(pkg));
        }
        for candidate in resolved {
            // TODO: Pretty print
            println!("{:?}", candidate.meta);
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client error: {0}")]
    Client(#[from] client::Error),

    #[error("no such package")]
    NotFound(String),
}
