// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::{arg, ArgMatches, Command};
use futures::{future::join_all, StreamExt};
use moss::{
    client::{self, Client},
    package::Flags,
    Package,
};
use thiserror::Error;

use crate::cli::name_to_provider;

use super::pretty::print_to_columns;

pub fn command() -> Command {
    Command::new("install")
        .about("Install packages")
        .long_about("Install the requested software to the local system")
        .arg(arg!(<NAME> ... "packages to install").value_parser(clap::value_parser!(String)))
}

/// Resolve a package ID into either an error or a set of packages matching
/// TODO: Collapse to .first() for installation selection
async fn find_packages(id: &str, client: &Client) -> Result<Vec<Package>, Error> {
    let provider = name_to_provider(id);
    let result = client
        .registry
        .by_provider(&provider, Flags::AVAILABLE)
        .collect::<Vec<_>>()
        .await;
    if result.is_empty() {
        return Err(Error::NoCandidate(id.to_string()));
    }
    Ok(result)
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

    let queried = join_all(pkgs.iter().map(|p| find_packages(p, &client))).await;
    let (good_list, bad_list): (Vec<_>, Vec<_>) = queried.into_iter().partition(Result::is_ok);
    let bad: Vec<_> = bad_list.into_iter().map(Result::unwrap_err).collect();
    let mut good: Vec<_> = good_list.into_iter().flat_map(Result::unwrap).collect();

    // TODO: Add error hookups
    if !bad.is_empty() {
        println!("Missing packages in lookup: {:?}", bad);
        return Err(Error::NotImplemented);
    }

    good.sort();
    good.dedup();

    println!("The following package(s) will be installed:");
    println!();
    print_to_columns(good);

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
