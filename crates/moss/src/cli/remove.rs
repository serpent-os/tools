// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::HashSet, path::Path};

use clap::{arg, ArgMatches, Command};
use futures::StreamExt;
use itertools::{Either, Itertools};
use moss::{
    client::{self, Client},
    environment,
    package::Flags,
    registry::transaction,
    state::Selection,
    Provider,
};
use thiserror::Error;
use tui::{
    dialoguer::{theme::ColorfulTheme, Confirm},
    pretty::print_to_columns,
    Stylize,
};

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
        .map(|name| Provider::from_name(name).unwrap())
        .collect::<Vec<_>>();
    let yes = *args.get_one::<bool>("yes").unwrap();

    // Grab a client for the target, enumerate packages
    let client = Client::new(environment::NAME, root).await?;

    let installed = client
        .registry
        .list_installed(Flags::NONE)
        .collect::<Vec<_>>()
        .await;
    let installed_ids = installed
        .iter()
        .map(|p| p.id.clone())
        .collect::<HashSet<_>>();

    // Separate packages between installed / not installed (or invalid)
    let (for_removal, not_installed): (Vec<_>, Vec<_>) = pkgs.iter().partition_map(|provider| {
        installed
            .iter()
            .find(|i| i.meta.providers.contains(provider))
            .map(|i| Either::Left(i.id.clone()))
            .unwrap_or(Either::Right(provider.clone()))
    });

    // Bail if there's packages not installed
    // TODO: Add error hookups
    if !not_installed.is_empty() {
        println!("Missing packages in lookup: {:?}", not_installed);
        return Err(Error::NotImplemented);
    }

    // Add all installed packages to transaction
    let mut transaction = client
        .registry
        .transaction_with_installed(installed_ids.clone().into_iter().collect())
        .await?;

    // Remove all pkgs for removal
    transaction.remove(for_removal).await;

    // Finalized tx has all reverse deps removed
    let finalized = transaction.finalize().cloned().collect::<HashSet<_>>();

    // Resolve all removed packages, where removed is (installed - finalized)
    let removed = client
        .resolve_packages(installed_ids.difference(&finalized))
        .await?;

    println!("The following package(s) will be removed:");
    println!();
    print_to_columns(&removed);
    println!();

    let result = if yes {
        true
    } else {
        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(" Do you wish to continue? ")
            .default(false)
            .interact()?
    };
    if !result {
        return Err(Error::Cancelled);
    }

    // Print each package to stdout
    for package in removed {
        println!(
            "{} {}",
            "Removed".red(),
            package.meta.name.to_string().bold(),
        );
    }

    // Map finalized state to a [`Selection`] by referencing
    // it's value from the previous state
    let new_state_pkgs = {
        let previous_selections = match client.installation.active_state {
            Some(id) => client.state_db.get(&id).await?.selections,
            None => vec![],
        };

        finalized
            .into_iter()
            .map(|id| {
                previous_selections
                    .iter()
                    .find(|s| s.package == id)
                    .cloned()
                    // Should be unreachable since new state from removal
                    // is always a subset of the previous state
                    .unwrap_or_else(|| {
                        eprintln!("Unreachable: previous selection not found during removal for package {id:?}, marking as not explicit");

                        Selection {
                            package: id,
                            explicit: false,
                            reason: None,
                        }
                    })
            })
            .collect::<Vec<_>>()
    };

    // Apply state
    client.apply_state(&new_state_pkgs, "Remove").await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("cancelled")]
    Cancelled,

    #[error("Not yet implemented")]
    NotImplemented,

    #[error("client")]
    Client(#[from] client::Error),

    #[error("transaction")]
    Transaction(#[from] transaction::Error),

    #[error("state db")]
    StateDB(#[from] moss::db::state::Error),

    #[error("io")]
    Io(#[from] std::io::Error),

    #[error("string processing")]
    Dialog(#[from] tui::dialoguer::Error),
}
