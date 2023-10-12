// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::HashSet, path::Path};

use clap::{arg, ArgMatches, Command};
use futures::{future::join_all, StreamExt};
use itertools::{Either, Itertools};
use moss::{
    client::{self, Client},
    package::Flags,
    registry::transaction,
};
use thiserror::Error;
use tui::pretty::print_to_columns;

use super::name_to_provider;

pub fn command() -> Command {
    Command::new("remove")
        .about("Remove packages")
        .long_about("Remove packages by name")
        .arg(arg!(<NAME> ... "packages to install").value_parser(clap::value_parser!(String)))
}

/// Handle execution of `moss remove`
pub async fn handle(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    let pkgs = args
        .get_many::<String>("NAME")
        .into_iter()
        .flatten()
        .map(|name| name_to_provider(name))
        .collect::<Vec<_>>();

    // Grab a client for the target, enumerate packages
    let client = Client::new_for_root(root).await?;

    let installed = client
        .registry
        .list_installed(Flags::NONE)
        .collect::<Vec<_>>()
        .await;
    let installed_ids = installed
        .iter()
        .map(|p| p.id.clone())
        .collect::<HashSet<_>>();

    let (for_removal, not_installed): (Vec<_>, Vec<_>) = pkgs.iter().partition_map(|provider| {
        installed
            .iter()
            .find(|i| i.meta.providers.contains(provider))
            .map(|i| Either::Left(i.id.clone()))
            .unwrap_or(Either::Right(provider.clone()))
    });

    // TODO: Add error hookups
    if !not_installed.is_empty() {
        println!("Missing packages in lookup: {:?}", not_installed);
        return Err(Error::NotImplemented);
    }

    let mut transaction = client.registry.transaction()?;

    // Add all installed packages to transaction
    transaction
        .add(installed_ids.iter().cloned().collect())
        .await?;

    // Remove all pkgs for removal
    transaction.remove(for_removal).await?;

    // Finalized tx has all reverse deps removed
    let finalized = transaction.finalize()?.into_iter().collect::<HashSet<_>>();

    // Difference resolves to all removed pkgs
    let removed = installed_ids.difference(&finalized);

    // Get metadata for all removed pkgs & dedupe
    let mut results = join_all(
        removed
            .into_iter()
            .map(|p| async { client.registry.by_id(p).boxed().next().await.unwrap() }),
    )
    .await;
    results.sort_by_key(|p| p.meta.name.to_string());
    results.dedup_by_key(|p| p.meta.name.to_string());

    println!("The following package(s) will be removed:");
    println!();
    print_to_columns(&results);
    println!();

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client error")]
    Client(#[from] client::Error),

    #[error("not yet implemented")]
    NotImplemented,

    #[error("transaction error: {0}")]
    Transaction(#[from] transaction::Error),
}
