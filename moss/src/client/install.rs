// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use futures::{future::join_all, StreamExt};
use thiserror::Error;
use tui::{
    dialoguer::{theme::ColorfulTheme, Confirm},
    pretty::print_to_columns,
};

use crate::{
    client::{self, Client},
    package::{self, Flags},
    registry::transaction,
    state::Selection,
    Package, Provider,
};

pub async fn install(client: &mut Client, pkgs: &[&str], yes: bool) -> Result<(), Error> {
    // Resolve input packages
    let input = resolve_input(pkgs, client).await?;

    // Add all inputs
    let mut tx = client.registry.transaction()?;

    tx.add(input.clone()).await?;

    // Resolve transaction to metadata
    let resolved = client.resolve_packages(tx.finalize()).await?;

    // Get installed packages to check against
    let installed = client.registry.list_installed(Flags::NONE).collect::<Vec<_>>().await;
    let is_installed = |p: &Package| installed.iter().any(|i| i.meta.name == p.meta.name);

    // Get missing packages that are:
    //
    // Stateful: Not installed
    // Ephemeral: all
    let missing = resolved
        .iter()
        .filter(|p| client.is_ephemeral() || !is_installed(p))
        .collect::<Vec<_>>();

    // If no new packages exist, exit and print
    // packages already installed
    if missing.is_empty() {
        let installed = resolved
            .iter()
            .filter(|p| is_installed(p) && input.contains(&p.id))
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

    // Cache packages
    client.cache_packages(&missing).await?;

    // Calculate the new state of packages (old_state + missing)
    let new_state_pkgs = {
        // Only use previous state in stateful mode
        let previous_selections = match client.installation.active_state {
            Some(id) if !client.is_ephemeral() => client.state_db.get(&id).await?.selections,
            _ => vec![],
        };
        let missing_selections = missing.iter().map(|p| Selection {
            package: p.id.clone(),
            // Package is explicit if it was one of the input
            // packages provided by the user
            explicit: input.iter().any(|id| *id == p.id),
            reason: None,
        });

        missing_selections.chain(previous_selections).collect::<Vec<_>>()
    };

    // Perfect, apply state.
    client.apply_state(&new_state_pkgs, "Install").await?;

    Ok(())
}

/// Resolves the package arguments as valid input packages. Returns an error
/// if any args are invalid.
async fn resolve_input(pkgs: &[&str], client: &Client) -> Result<Vec<package::Id>, Error> {
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
    let provider = Provider::from_name(id).unwrap();
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
    InstallDB(#[from] crate::db::meta::Error),

    #[error("layout db")]
    LayoutDB(#[from] crate::db::layout::Error),

    #[error("state db")]
    StateDB(#[from] crate::db::state::Error),

    #[error("string processing")]
    Dialog(#[from] tui::dialoguer::Error),

    #[error("io")]
    Io(#[from] std::io::Error),
}
