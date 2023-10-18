// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use itertools::Itertools;
use thiserror::Error;
use tui::pretty::print_to_columns;

use crate::{db, package, state};

/// The prune strategy for removing old states
#[derive(Debug, Clone, Copy)]
pub enum Strategy {
    /// Keep the most recent N states, remove the rest
    KeepRecent(u64),
    /// Removes a specific state
    Remove(state::Id),
}

/// The status of a state
enum Status {
    /// Keep the state
    Keep(state::Id),
    /// Remove the state
    Remove(state::Id),
}

impl Status {
    fn id(&self) -> &state::Id {
        match self {
            Status::Keep(id) => id,
            Status::Remove(id) => id,
        }
    }

    fn is_removal(&self) -> bool {
        matches!(self, Self::Remove(_))
    }
}

/// Prune old states using [`Strategy`] and garbage collect
/// all cached data related to those states being removed
///
/// TODO: Add indicatif / CLI output
pub async fn prune(
    strategy: Strategy,
    state_db: &db::state::Database,
    install_db: &db::meta::Database,
    layout_db: &db::layout::Database,
) -> Result<(), Error> {
    let state_ids = state_db.list_ids().await?;

    // Define each state as either Keep or Remove
    let states_by_status = match strategy {
        Strategy::KeepRecent(keep) => {
            // Calculate how many states over the limit we are
            let num_to_remove = state_ids.len().saturating_sub(keep as usize);

            state_ids
                .into_iter()
                .sorted_by_key(|(_, created)| *created)
                .enumerate()
                .map(|(idx, (id, _))| {
                    if idx < num_to_remove {
                        Status::Remove(id)
                    } else {
                        Status::Keep(id)
                    }
                })
                .collect::<Vec<_>>()
        }
        Strategy::Remove(remove) => state_ids
            .iter()
            .find_map(|(id, _)| (*id == remove).then_some(Status::Remove(remove)))
            .into_iter()
            .collect(),
    };

    if !states_by_status.iter().any(Status::is_removal) {
        // TODO: Print no states to be removed
        return Ok(());
    }

    // Keep track of how many active states are using a package
    let mut packages_counts = HashMap::<package::Id, usize>::new();
    let mut removals = vec![];

    // Add each package and get net count
    for status in states_by_status {
        // Get metadata
        let state = state_db.get(status.id()).await?;

        dbg!(&state.packages.len());

        // Increment each package
        state.packages.iter().for_each(|pkg| {
            *packages_counts.entry(pkg.clone()).or_default() += 1;
        });

        // Decrement if removal
        if status.is_removal() {
            state.packages.iter().for_each(|pkg| {
                *packages_counts.entry(pkg.clone()).or_default() -= 1;
            });
            removals.push(state);
        }
    }

    println!("The following state(s) will be removed:");
    println!();
    print_to_columns(
        &removals
            .iter()
            .map(state::ColumnDisplay)
            .collect::<Vec<_>>(),
    );
    println!();

    // Get all packages which were decremented to 0
    let package_removals = packages_counts
        .into_iter()
        .filter_map(|(pkg, count)| (count == 0).then_some(pkg))
        .collect::<Vec<_>>();

    dbg!(package_removals);

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("state db: {0}")]
    StateDB(#[from] db::state::Error),
}
