// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
};

use futures::{stream, Future, FutureExt, StreamExt, TryStreamExt};
use itertools::Itertools;
use thiserror::Error;
use tokio::{fs, task};
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

            // Sort ascending and assign first `num_to_remove` as `Status::Remove`
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
            // Remove if this id actually exists
            .find_map(|(id, _)| (*id == remove).then_some(Status::Remove(remove)))
            .into_iter()
            .collect(),
    };

    // Bail if there's no states to remove
    if !states_by_status.iter().any(Status::is_removal) {
        // TODO: Print no states to be removed
        return Ok(());
    }

    // Keep track of how many active states are using a package
    let mut packages_counts = HashMap::<package::Id, usize>::new();
    // Collects the states we will remove
    let mut removals = vec![];

    // Get net refcount of each package and collect removal states
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

    // Get all packages which were decremented to 0,
    // these are the packages we want to remove since
    // no more states reference them
    let package_removals = packages_counts
        .into_iter()
        .filter_map(|(pkg, count)| (count == 0).then_some(pkg))
        .collect::<Vec<_>>();

    // Print out the states to be removed to the user
    println!("The following state(s) will be removed:");
    println!();
    print_to_columns(
        &removals
            .iter()
            .map(state::ColumnDisplay)
            .collect::<Vec<_>>(),
    );
    println!();

    // Prune these states / packages from all dbs
    {
        // Remove db states
        state_db
            .batch_remove(removals.iter().map(|state| &state.id))
            .await?;
        // Remove db metadata
        install_db.batch_remove(&package_removals).await?;
        // Remove db layouts
        layout_db.batch_remove(&package_removals).await?;
    }

    // Remove orphaned downloads
    remove_orphaned_files(
        // root
        installation.cache_path("downloads").join("v1"),
        // final set of hashes to compare against
        install_db.file_hashes().await?,
        // path builder using hash
        |hash| async move {
            package::fetch::download_path(installation, &hash)
                .map(Result::ok)
                .await
        },
    )
    .await?;

    // Remove orphaned assets
    remove_orphaned_files(
        // root
        installation.assets_path("v2"),
        // final set of hashes to compare against
        layout_db.file_hashes().await?,
        // path builder using hash
        |hash| async move {
            package::fetch::asset_path(installation, &hash)
                .map(Result::ok)
                .await
        },
    )
    .await?;

    Ok(())
}

/// Removes all files under `root` that no longer exist in the provided `final_hashes` set
async fn remove_orphaned_files<F>(
    root: PathBuf,
    final_hashes: HashSet<String>,
    compute_path: impl Fn(String) -> F,
) -> Result<(), Error>
where
    F: Future<Output = Option<PathBuf>>,
{
    // Compute hashes to remove by (installed - final)
    let installed_hashes = enumerate_file_hashes(&root).await?;
    let hashes_to_remove = installed_hashes.difference(&final_hashes);

    // Remove each and it's parent dir if empty
    stream::iter(hashes_to_remove)
        .map(|hash| async {
            // Compute path to file using hash
            let Some(file) = compute_path(hash.clone()).await else {
                return Ok(());
            };

            // Remove if it exists
            if fs::try_exists(&file).await? {
                fs::remove_file(&file).await?;
            }

            // Try to remove leading parent dirs if they're
            // now empty
            if let Some(parent) = file.parent() {
                let _ = remove_empty_dirs(parent, &root).await;
            }

            Ok(()) as Result<(), Error>
        })
        // Remove w/ concurrency!
        .buffer_unordered(CONCURRENT_REMOVALS)
        .try_collect::<()>()
        .await?;

    Ok(())
}

/// Returns all nested files under `root` and parses the file name as a hash
async fn enumerate_file_hashes(root: impl Into<PathBuf>) -> Result<HashSet<String>, io::Error> {
    let files = enumerate_files(root).await?;

    let path_to_hash = |path: PathBuf| {
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string()
    };

    Ok(files.into_iter().map(path_to_hash).collect())
}

/// Returns all nested files under `root`
async fn enumerate_files(root: impl Into<PathBuf>) -> Result<Vec<PathBuf>, io::Error> {
    use std::fs;

    use rayon::prelude::*;

    fn recurse(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
        let mut dirs = vec![];
        let mut files = vec![];

        let contents = fs::read_dir(dir)?;

        for entry in contents {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let path = entry.path();

            if file_type.is_dir() {
                dirs.push(path);
            } else if file_type.is_file() {
                files.push(path);
            }
        }

        let nested_files = dirs
            .par_iter()
            .map(recurse)
            .try_reduce(Vec::new, |acc, files| {
                Ok(acc.into_iter().chain(files).collect())
            })?;

        Ok(files.into_iter().chain(nested_files).collect())
    }

    let root = root.into();

    task::spawn_blocking(|| recurse(root))
        .await
        .expect("join handle")
}

/// Remove all empty folders from `starting` and moving up until `root`
///
/// `root` must be a prefix / ancestor of `starting`
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
