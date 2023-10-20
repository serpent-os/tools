// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use clap::{arg, ArgMatches, Command};
use futures::{future::join_all, StreamExt};
use itertools::Itertools;
use moss::{
    client::{self, Client},
    package::{self, Flags},
    registry::transaction,
    Package,
};
use thiserror::Error;
use tui::pretty::print_to_columns;

use crate::cli::name_to_provider;

pub fn command() -> Command {
    Command::new("install")
        .about("Install packages")
        .long_about("Install the requested software to the local system")
        .arg(arg!(<NAME> ... "packages to install").value_parser(clap::value_parser!(String)))
}

/// Handle execution of `moss install`
pub async fn handle(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    let pkgs = args
        .get_many::<String>("NAME")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    // Grab a client for the root
    let client = Client::new_for_root(root).await?;

    // Resolve input packages
    let input = resolve_input(pkgs, &client).await?;

    // Add all inputs
    let mut tx = client.registry.transaction()?;
    tx.add(input.clone()).await?;

    // Resolve transaction to metadata
    let resolved = client.resolve_packages(tx.finalize()).await?;

    // Get missing packages that aren't installed
    let missing = resolved
        .iter()
        .filter(|p| !p.is_installed())
        .collect::<Vec<_>>();

    // If no new packages exist, exit and print
    // packages already installed
    if missing.is_empty() {
        let installed = resolved
            .iter()
            .filter(|p| p.is_installed() && input.contains(&p.id))
            .collect::<Vec<_>>();

        if !installed.is_empty() {
            println!("The following package(s) are already installed:");
            println!();
            print_to_columns(&installed);
        }

        return Ok(());
    }

    println!("The following package(s) will be installed:");
    println!();
    print_to_columns(&missing);
    println!();

    // Cache packages
    client.cache_packages(&missing).await?;

    // Calculate the new state of packages (old_state + missing)
    let new_state_pkgs = {
        let previous_state_pkgs = match client.installation.active_state {
            Some(id) => client.state_db.get(&id).await?.packages,
            None => vec![],
        };
        missing
            .iter()
            .map(|p| p.id.clone())
            .chain(previous_state_pkgs)
            .collect::<Vec<_>>()
    };

    // Perfect, record state.
    client.record_state(&new_state_pkgs, "Install").await?;

    Err(Error::NotImplemented)
}

/// Resolves the pacakge arguments as valid input packages. Returns an error
/// if any args are invalid.
async fn resolve_input(pkgs: Vec<String>, client: &Client) -> Result<Vec<package::Id>, Error> {
    // Parse pkg args into valid / invalid sets
    let queried = join_all(pkgs.iter().map(|p| find_packages(p, client))).await;
    let (valid_pkgs, invalid_pkgs): (Vec<_>, Vec<_>) = queried.into_iter().partition_result();

    // TODO: Add error hookups
    if !invalid_pkgs.is_empty() {
        println!("Missing packages in lookup: {:?}", invalid_pkgs);
        return Err(Error::NotImplemented);
    }

    Ok(valid_pkgs.into_iter().flatten().map(|p| p.id).collect())
}

/// Resolve a package ID into either an error or a set of packages matching
/// TODO: Collapse to .first() for installation selection
async fn find_packages<'a>(id: &'a str, client: &Client) -> Result<Vec<Package>, &'a str> {
    let provider = name_to_provider(id);
    let result = client
        .registry
        .by_provider(&provider, Flags::AVAILABLE)
        .collect::<Vec<_>>()
        .await;
    if result.is_empty() {
        return Err(id);
    }
    Ok(result)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Not yet implemented")]
    NotImplemented,

    #[error("client")]
    Client(#[from] client::Error),

    #[error("transaction")]
    Transaction(#[from] transaction::Error),

    #[error("install db")]
    InstallDB(#[from] moss::db::meta::Error),

    #[error("layout db")]
    LayoutDB(#[from] moss::db::layout::Error),

    #[error("state db")]
    StateDB(#[from] moss::db::state::Error),

    #[error("io")]
    Io(#[from] std::io::Error),
}
