// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::borrow::Cow;
use std::path::PathBuf;
use std::{collections::BTreeSet, path::Path};

use clap::{arg, value_parser, ArgMatches, Command};
use futures::{stream, StreamExt, TryStreamExt};
use moss::environment;
use moss::registry::transaction;
use moss::state::Selection;
use moss::{
    client::{self, Client},
    package::{self, Flags},
    Package,
};
use thiserror::Error;

use tui::dialoguer::theme::ColorfulTheme;
use tui::dialoguer::Confirm;
use tui::pretty::print_to_columns;

pub fn command() -> Command {
    Command::new("sync")
        .about("Sync packages")
        .long_about("Sync package selections with candidates from the highest priority repository")
        .arg(arg!(--"upgrade-only" "Only sync packages that have a version upgrade"))
        .arg(
            arg!(--to <blit_target> "Blit this sync to the provided directory instead of the root")
                .long_help(
                    "Blit this sync to the provided directory instead of the root. \n\
                     \n\
                     This operation won't be captured as a new state",
                )
                .value_parser(value_parser!(PathBuf)),
        )
}

pub async fn handle(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    let yes_all = *args.get_one::<bool>("yes").unwrap();
    let upgrade_only = *args.get_one::<bool>("upgrade-only").unwrap();

    let mut client = Client::new(environment::NAME, root).await?;

    // Make ephemeral if a blit target was provided
    if let Some(blit_target) = args.get_one::<PathBuf>("to").cloned() {
        client = client.ephemeral(blit_target)?;
    }

    // Grab all the existing installed packages
    let installed = client
        .registry
        .list_installed(package::Flags::NONE)
        .collect::<Vec<_>>()
        .await;
    if installed.is_empty() {
        return Err(Error::NoInstall);
    }

    // Resolve the finalized state w/ 2 passes.
    //
    // 1. Resolve a new state based on all explicit packages with sync applied
    // 2. Resolve a new state based on `1`, this ensures applicable transitive
    //    sync is applied
    //
    // By resolving only explicit first, this ensures any "orphaned" transitive deps
    // are naturally dropped from the final state.
    let first_pass =
        resolve_with_sync(&client, Resolution::Explicit, upgrade_only, &installed).await?;
    let finalized = resolve_with_sync(&client, Resolution::All, upgrade_only, &first_pass).await?;

    // Synced are packages are:
    //
    // Stateful: Not installed
    // Ephemeral: All
    let synced = finalized
        .iter()
        .filter(|p| client.is_ephemeral() || !installed.iter().any(|i| i.id == p.id))
        .collect::<Vec<_>>();
    let removed = installed
        .iter()
        .filter(|p| !finalized.iter().any(|f| f.meta.name == p.meta.name))
        .cloned()
        .collect::<Vec<_>>();

    if synced.is_empty() && removed.is_empty() {
        println!("No packages to sync");
        return Ok(());
    }

    if !synced.is_empty() {
        println!("The following packages will be sync'd: ");
        println!();
        print_to_columns(synced.as_slice());
        println!();
    }
    if !removed.is_empty() {
        println!("The following orphaned packages will be removed: ");
        println!();
        print_to_columns(removed.as_slice());
        println!();
    }

    // Must we prompt?
    let result = if yes_all {
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

    client.cache_packages(&synced).await?;

    // Map finalized state to a [`Selection`] by referencing
    // it's value from the previous state
    let new_selections = {
        let previous_selections = match client.installation.active_state {
            Some(id) => client.state_db.get(&id).await?.selections,
            None => vec![],
        };

        finalized
            .into_iter()
            .map(|p| {
                // Use old version id to lookup previous selection
                let lookup_id = installed
                    .iter()
                    .find_map(|i| (i.meta.name == p.meta.name).then_some(&i.id))
                    .unwrap_or(&p.id);

                previous_selections
                    .iter()
                    .find(|s| s.package == *lookup_id)
                    .cloned()
                    // Use prev reason / explicit flag & new id
                    .map(|s| Selection {
                        package: p.id.clone(),
                        ..s
                    })
                    // Must be transitive
                    .unwrap_or(Selection {
                        package: p.id,
                        explicit: false,
                        reason: None,
                    })
            })
            .collect::<Vec<_>>()
    };

    // Perfect, apply state.
    client.apply_state(&new_selections, "Sync").await?;

    Ok(())
}

enum Resolution {
    Explicit,
    All,
}

/// Return a fully resolved package set w/ sync'd changes swapped in
/// using the provided `packages` at the requested [`Resolution`]
async fn resolve_with_sync(
    client: &Client,
    resolution: Resolution,
    upgrade_only: bool,
    packages: &[Package],
) -> Result<Vec<Package>, Error> {
    let all_ids = packages.iter().map(|p| &p.id).collect::<BTreeSet<_>>();

    // For each package, replace it w/ it's sync'd change (if available)
    // or return the original package
    let with_sync = stream::iter(packages.iter())
        .filter(|p| async {
            match resolution {
                Resolution::Explicit => p.flags.contains(Flags::EXPLICIT),
                Resolution::All => true,
            }
        })
        .map(|p| async {
            // Get first available = use highest priority
            if let Some(lookup) = client
                .registry
                .by_name(&p.meta.name, package::Flags::AVAILABLE)
                .boxed()
                .next()
                .await
            {
                let upgrade_check = if upgrade_only {
                    lookup.meta.source_release > p.meta.source_release
                } else {
                    true
                };

                if !all_ids.contains(&lookup.id) && upgrade_check {
                    Ok(Cow::Owned(lookup))
                } else {
                    Ok(Cow::Borrowed(p))
                }
            } else {
                Err(Error::NameNotFound(p.meta.name.clone()))
            }
        })
        .buffer_unordered(environment::MAX_DISK_CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?;

    // Build a new tx from this sync'd package set
    let mut tx = client.registry.transaction()?;
    tx.add(with_sync.iter().map(|p| p.id.clone()).collect())
        .await?;

    // Resolve the tx
    Ok(client.resolve_packages(tx.finalize()).await?)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("cancelled")]
    Cancelled,

    #[error("unknown package name")]
    NameNotFound(package::Name),

    #[error("no installation")]
    NoInstall,

    #[error("client")]
    Client(#[from] client::Error),

    #[error("state db")]
    StateDB(#[from] moss::db::state::Error),

    #[error("string processing")]
    Dialog(#[from] tui::dialoguer::Error),

    #[error("transaction")]
    Transaction(#[from] transaction::Error),

    #[error("io")]
    Io(#[from] std::io::Error),
}
