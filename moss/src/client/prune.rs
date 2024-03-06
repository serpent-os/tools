// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use itertools::Itertools;
use thiserror::Error;
use tui::pretty::print_to_columns;

use crate::{client::cache, db, environment, package, state, Installation, State};

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
pub fn prune(
    strategy: Strategy,
    state_db: &db::state::Database,
    install_db: &db::meta::Database,
    layout_db: &db::layout::Database,
    installation: &Installation,
) -> Result<(), Error> {
    // Only prune if the moss root has an active state (otherwise
    // it's probably borked or not setup yet)
    let Some(current_state) = installation.active_state else {
        return Err(Error::NoActiveState);
    };

    let state_ids = state_db.list_ids()?;

    // Define each state as either Keep or Remove
    let states_by_status = match strategy {
        Strategy::KeepRecent(keep) => {
            // Filter for all states before the current
            let old_states = state_ids
                .iter()
                .filter(|(id, _)| *id < current_state)
                .collect::<Vec<_>>();
            // Deduct current state from num to keep
            let old_limit = (keep as usize).saturating_sub(1);

            // Calculate how many old states over the limit we are
            let num_to_remove = old_states.len().saturating_sub(old_limit);

            // Sort ascending and assign first `num_to_remove` as `Status::Remove`
            old_states
                .into_iter()
                .sorted_by_key(|(_, created)| *created)
                .enumerate()
                .map(|(idx, (id, _))| {
                    if idx < num_to_remove {
                        Status::Remove(*id)
                    } else {
                        Status::Keep(*id)
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
        let state = state_db.get(status.id())?;

        // Increment each package
        state.selections.iter().for_each(|selection| {
            *packages_counts.entry(selection.package.clone()).or_default() += 1;
        });

        // Decrement if removal
        if status.is_removal() {
            // Ensure we're not pruning the active state!!
            if status.id() == &current_state {
                return Err(Error::PruneCurrent);
            }

            state.selections.iter().for_each(|selection| {
                *packages_counts.entry(selection.package.clone()).or_default() -= 1;
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
    print_to_columns(&removals.iter().map(state::ColumnDisplay).collect::<Vec<_>>());
    println!();

    // Prune these states / packages from all dbs
    prune_databases(&removals, &package_removals, state_db, install_db, layout_db)?;

    // Remove orphaned downloads
    remove_orphaned_files(
        // root
        installation.cache_path("downloads").join("v1"),
        // final set of hashes to compare against
        install_db.file_hashes()?,
        // path builder using hash
        |hash| cache::download_path(installation, &hash).ok(),
    )?;

    // Remove orphaned assets
    remove_orphaned_files(
        // root
        installation.assets_path("v2"),
        // final set of hashes to compare against
        layout_db.file_hashes()?,
        // path builder using hash
        |hash| Some(cache::asset_path(installation, &hash)),
    )?;

    Ok(())
}

/// Removes the provided states & packages from the databases
fn prune_databases(
    states: &[State],
    packages: &[package::Id],
    state_db: &db::state::Database,
    install_db: &db::meta::Database,
    layout_db: &db::layout::Database,
) -> Result<(), Error> {
    for chunk in &states.iter().map(|state| &state.id).chunks(environment::DB_BATCH_SIZE) {
        // Remove db states
        state_db.batch_remove(chunk)?;
    }
    for chunk in &packages.iter().chunks(environment::DB_BATCH_SIZE) {
        // Remove db metadata
        install_db.batch_remove(chunk)?;
    }
    for chunk in &packages.iter().chunks(environment::DB_BATCH_SIZE) {
        // Remove db layouts
        layout_db.batch_remove(chunk)?;
    }

    Ok(())
}

/// Removes all files under `root` that no longer exist in the provided `final_hashes` set
fn remove_orphaned_files(
    root: PathBuf,
    final_hashes: HashSet<String>,
    compute_path: impl Fn(String) -> Option<PathBuf>,
) -> Result<(), Error> {
    // Compute hashes to remove by (installed - final)
    let installed_hashes = enumerate_file_hashes(&root)?;
    let hashes_to_remove = installed_hashes.difference(&final_hashes);

    // Remove each and it's parent dir if empty
    hashes_to_remove.into_iter().try_for_each(|hash| {
        // Compute path to file using hash
        let Some(file) = compute_path(hash.clone()) else {
            return Ok(());
        };

        // Remove if it exists
        if file.exists() {
            fs::remove_file(&file)?;
        }

        // Try to remove leading parent dirs if they're
        // now empty
        if let Some(parent) = file.parent() {
            let _ = remove_empty_dirs(parent, &root);
        }

        Ok(()) as Result<(), Error>
    })?;

    Ok(())
}

/// Returns all nested files under `root` and parses the file name as a hash
fn enumerate_file_hashes(root: impl AsRef<Path>) -> Result<HashSet<String>, io::Error> {
    let files = enumerate_files(root)?;

    let path_to_hash = |path: PathBuf| {
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string()
    };

    Ok(files.into_iter().map(path_to_hash).collect())
}

/// Returns all nested files under `root`
fn enumerate_files(root: impl AsRef<Path>) -> Result<Vec<PathBuf>, io::Error> {
    use rayon::prelude::*;

    fn recurse(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
        let mut dirs = vec![];
        let mut files = vec![];

        if !dir.as_ref().exists() {
            return Ok(vec![]);
        }

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
            .try_reduce(Vec::new, |acc, files| Ok(acc.into_iter().chain(files).collect()))?;

        Ok(files.into_iter().chain(nested_files).collect())
    }

    recurse(root)
}

/// Remove all empty folders from `starting` and moving up until `root`
///
/// `root` must be a prefix / ancestor of `starting`
fn remove_empty_dirs(starting: &Path, root: &Path) -> Result<(), io::Error> {
    if !starting.starts_with(root) || !starting.is_dir() || !root.is_dir() {
        return Ok(());
    }

    let mut current = Some(starting);

    while let Some(dir) = current.take() {
        if dir.exists() {
            let is_empty = fs::read_dir(dir)?.count() == 0;

            if !is_empty {
                return Ok(());
            }

            fs::remove_dir(dir)?;
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
    #[error("no active state found")]
    NoActiveState,
    #[error("cannot prune the currently active state")]
    PruneCurrent,
    #[error("layout db")]
    LayoutDB(#[from] db::layout::Error),
    #[error("meta db")]
    MetaDB(#[from] db::meta::Error),
    #[error("state db")]
    StateDB(#[from] db::state::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
