// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::{arg, ArgMatches, Command};
use futures::StreamExt;
use moss::{
    client::{self, Client},
    package::Flags,
};
use thiserror::Error;

use crate::cli::name_to_provider;

pub fn command() -> Command {
    Command::new("install")
        .about("Install packages")
        .long_about("Install the requested software to the local system")
        .arg(arg!(<NAME> ... "packages to install").value_parser(clap::value_parser!(String)))
}

/// Handle execution of `moss install`
pub async fn handle(args: &ArgMatches) -> Result<(), Error> {
    let root = args.get_one::<PathBuf>("root").unwrap().clone();

    let pkgs = args
        .get_many::<String>("NAME")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    // Grab a client for the target, enumerate packages
    let client = Client::new_for_root(root).await?;
    let mut requested = vec![];

    for pkg in pkgs {
        let lookup = name_to_provider(&pkg);

        let result = client
            .registry
            .by_provider(&lookup, Flags::AVAILABLE)
            .collect::<Vec<_>>()
            .await;
        if result.is_empty() {
            return Err(Error::NoCandidate(pkg));
        }
        let front = result.first().unwrap();
        requested.push(front.meta.id().clone());
    }
    requested.sort_by_key(|i| i.to_string());
    requested.dedup();

    println!("Candidates: {:?}", requested);

    Err(Error::NotImplemented)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client error")]
    Client(#[from] client::Error),

    #[error("no such candidate: {0}")]
    NoCandidate(String),

    #[error("not yet implemented")]
    NotImplemented,
}
