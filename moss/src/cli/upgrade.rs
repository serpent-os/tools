// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::BTreeSet, path::Path};

use clap::{ArgMatches, Command};
use futures::StreamExt;
use moss::{
    client::{self, Client},
    package,
};
use thiserror::Error;
use tui::pretty::print_to_columns;

pub fn command() -> Command {
    Command::new("upgrade")
        .about("Upgrade the system")
        .long_about("Upgrade all packages to their latest versions")
}

pub async fn handle(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    let client = Client::new_for_root(root).await?;
    let _yes_all = args.get_one::<bool>("yes").unwrap();

    // Grab all the existing installed packages
    // TODO: Filter by non transitive
    let installed_packages = client
        .registry
        .list_installed(package::Flags::NONE)
        .collect::<Vec<_>>()
        .await;
    if installed_packages.is_empty() {
        return Err(Error::NoInstall);
    }

    let installed_ids = installed_packages
        .iter()
        .map(|p| p.id.clone())
        .collect::<BTreeSet<_>>();
    let names = installed_packages
        .iter()
        .map(|p| p.meta.name.clone())
        .collect::<Vec<_>>();

    // Explicit "Upgrades"
    let mut upgrades = vec![];
    // Full set
    let mut new_state = vec![];

    for name in names.iter() {
        // pull only the first Available, don't really care for installed.
        if let Some(lookup) = client
            .registry
            .by_name(name, package::Flags::AVAILABLE)
            .boxed()
            .next()
            .await
        {
            if !installed_ids.contains(&lookup.id) {
                upgrades.push(lookup.id.clone());
            }
            new_state.push(lookup.id.clone());
        } else {
            return Err(Error::NameNotFound(name.clone()));
        }
    }

    // Resolve to usable set
    let upgrades = client.resolve_packages(upgrades.iter()).await?;
    if upgrades.is_empty() {
        println!("No packages available for upgrade");
        Ok(())
    } else {
        println!("The following packages will be upgraded: ");
        print_to_columns(upgrades.as_slice());

        Err(Error::NotImplemented)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client")]
    Client(#[from] client::Error),

    #[error("unknown package name")]
    NameNotFound(package::Name),

    #[error("no installation")]
    NoInstall,

    #[error("not implemented")]
    NotImplemented,
}
