// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use clap::{arg, ArgMatches, Command};
use futures::{future::join_all, StreamExt};
use moss::{
    client::{self, Client},
    package::{self, Flags},
    registry::transaction,
    state::Selection,
    Package,
};
use thiserror::Error;
use tui::{ask_yes_no, pretty::print_to_columns};

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
    let client = Client::new(root).await?;
    let active_state = client.installation.active_state;

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

    // Must we prompt?
    if args.get_one::<bool>("yes").unwrap() == &false && !ask_yes_no("Do you wish to continue?")? {
        return Err(Error::Cancelled);
    }

    // Cache packages
    client.cache_packages(&missing).await?;

    // Calculate the new state of packages (old_state + missing)
    let new_state_pkgs = {
        let previous_selections = match active_state {
            Some(id) => client.state_db.get(&id).await?.selections,
            None => vec![],
        };
        let missing_selections = missing.iter().map(|p| Selection {
            package: p.id.clone(),
            // Package is explicit if it was one of the input
            // packages provided by the user
            explicit: input.iter().any(|id| *id == p.id),
            reason: None,
        });

        missing_selections
            .chain(previous_selections)
            .collect::<Vec<_>>()
    };

    // Perfect, apply state.
    client.apply_state(&new_state_pkgs, "Install").await?;

    Ok(())
}

/// Resolves the package arguments as valid input packages. Returns an error
/// if any args are invalid.
async fn resolve_input(pkgs: Vec<String>, client: &Client) -> Result<Vec<package::Id>, Error> {
    // Parse pkg args into valid / invalid sets
    let queried = join_all(pkgs.iter().map(|p| find_packages(p, client))).await;

    let mut results = vec![];

    for (id, pkg) in queried {
        if let Some(pkg) = pkg {
            results.push(pkg.id.clone())
        } else {
            return Err(Error::NoPackage(id));
        }
    }

    Ok(results)
}

/// Resolve a package name to the first package
async fn find_packages<'a>(id: &'a str, client: &Client) -> (String, Option<Package>) {
    let provider = name_to_provider(id);
    let result = client
        .registry
        .by_provider(&provider, Flags::AVAILABLE)
        .collect::<Vec<_>>()
        .await;

    // First only, pre-sorted
    (id.into(), result.first().cloned())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("cancelled")]
    Cancelled,

    #[error("client")]
    Client(#[from] client::Error),

    #[error("no package found: {0}")]
    NoPackage(String),

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
