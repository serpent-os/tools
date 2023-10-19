// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::{HashMap, HashSet},
    io,
    path::Path,
};

use futures::{stream, StreamExt, TryStreamExt};
use itertools::Itertools;
use thiserror::Error;
use tokio::fs;
use tui::pretty::print_to_columns;

use crate::{db, package, state, Installation};

const CONCURRENT_REMOVALS: usize = 16;

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
pub async fn prune(
    strategy: Strategy,
    state_db: &db::state::Database,
    install_db: &db::meta::Database,
    layout_db: &db::layout::Database,
    installation: &Installation,
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

    let download_hashes = stream::iter(package_removals.iter())
        .then(|id| install_db.get(id))
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .filter_map(|meta| meta.hash)
        .collect::<HashSet<_>>();

    // Remove states
    state_db
        .batch_remove(removals.iter().map(|state| &state.id))
        .await?;

    // Remove metadata
    install_db.batch_remove(&package_removals).await?;

    // Remove layouts and compute change in file hashes
    let pre_hashes = layout_db.file_hashes().await?;
    layout_db.batch_remove(&package_removals).await?;
    let post_hashes = layout_db.file_hashes().await?;
    let asset_hashes = pre_hashes.difference(&post_hashes);

    // Remove cached assets
    stream::iter(asset_hashes)
        .map(|hash| async {
            let hash = format!("{:02x}", *hash);
            let Ok(asset) = package::fetch::asset_path(installation, &hash).await else {
                return Ok(());
            };
            if fs::try_exists(&asset).await? {
                fs::remove_file(&asset).await?;
            }

            if let Some(parent) = asset.parent() {
                remove_empty_dirs(parent, &installation.assets_path("v2")).await?;
            }

            Ok(()) as Result<(), Error>
        })
        .buffer_unordered(CONCURRENT_REMOVALS)
        .try_collect::<()>()
        .await?;

    // Remove cached downloads
    stream::iter(&download_hashes)
        .map(|hash| async {
            let Ok(download) = package::fetch::download_path(installation, hash).await else {
                return Ok(());
            };
            if fs::try_exists(&download).await? {
                fs::remove_file(&download).await?;
            }

            if let Some(parent) = download.parent() {
                remove_empty_dirs(parent, &installation.cache_path("downloads").join("v1")).await?;
            }

            Ok(()) as Result<(), Error>
        })
        .buffer_unordered(CONCURRENT_REMOVALS)
        .try_collect::<()>()
        .await?;

    Ok(())
}

/// Remove all empty folders from `starting` and moving up until `root`
///
/// `root` must be a prefix / ancestory of `starting`
async fn remove_empty_dirs(starting: &Path, root: &Path) -> Result<(), io::Error> {
    if !starting.starts_with(root) || !starting.is_dir() || !root.is_dir() {
        return Ok(());
    }

    let mut current = Some(starting);

    while let Some(dir) = current.take() {
        if fs::try_exists(dir).await? {
            let is_empty = fs::read_dir(&dir).await?.next_entry().await?.is_none();

            if !is_empty {
                return Ok(());
            }

            fs::remove_dir(&dir).await?;
        }

        if let Some(parent) = dir.parent() {
            if parent != root {
                current = Some(parent);
            }
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("layout db: {0}")]
    LayoutDB(#[from] db::layout::Error),
    #[error("meta db: {0}")]
    MetaDB(#[from] db::meta::Error),
    #[error("state db: {0}")]
    StateDB(#[from] db::state::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}
