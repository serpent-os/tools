// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Boot management integration in moss

use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
    vec,
};

use blsforme::{
    os_release::{self, OsRelease},
    CmdlineEntry, Entry, Schema,
};
use fnmatch::Pattern;
use fs_err as fs;
use itertools::Itertools;
use stone::payload::layout::{self, Layout};
use thiserror::{self, Error};

use crate::{db, package::Id, Installation, State};

use super::Client;

#[derive(Debug, Error)]
pub enum Error {
    #[error("blsforme: {0}")]
    Blsforme(#[from] blsforme::Error),

    #[error("layoutdb: {0}")]
    Client(#[from] db::layout::Error),

    #[error("io: {0}")]
    IO(#[from] io::Error),

    #[error("os_release: {0}")]
    OsRelease(#[from] os_release::Error),

    /// fnmatch pattern compilation for boot, etc.
    #[error("fnmatch pattern: {0}")]
    Pattern(#[from] fnmatch::Error),

    #[error("incomplete kernel tree: {0}")]
    IncompleteKernel(String),
}

/// Simple mapping type for kernel discovery paths, retaining the layout reference
#[derive(Debug)]
struct KernelCandidate {
    path: PathBuf,
    _layout: Layout,
}

impl AsRef<Path> for KernelCandidate {
    fn as_ref(&self) -> &Path {
        self.path.as_path()
    }
}

/// From a given set of input paths, produce a set of match pairs
/// This is applied against the given system root
fn kernel_files_from_state<'a>(layouts: &'a [(Id, Layout)], pattern: &'a Pattern) -> Vec<KernelCandidate> {
    let mut kernel_entries = vec![];

    for (_, path) in layouts.iter() {
        match &path.entry {
            layout::Entry::Regular(_, target) => {
                if pattern.match_path(target).is_some() {
                    kernel_entries.push(KernelCandidate {
                        path: PathBuf::from("usr").join(target),
                        _layout: path.to_owned(),
                    });
                }
            }
            layout::Entry::Symlink(_, target) => {
                if pattern.match_path(target).is_some() {
                    kernel_entries.push(KernelCandidate {
                        path: PathBuf::from("usr").join(target),
                        _layout: path.to_owned(),
                    });
                }
            }
            _ => {}
        }
    }

    kernel_entries
}

/// Find bootloader assets in the new state
fn boot_files_from_new_state<'a>(
    install: &Installation,
    layouts: &'a [(Id, Layout)],
    pattern: &'a Pattern,
) -> Vec<PathBuf> {
    let mut rets = vec![];

    for (_, path) in layouts.iter() {
        if let layout::Entry::Regular(_, target) = &path.entry {
            if pattern.match_path(target).is_some() {
                rets.push(install.root.join("usr").join(target));
            }
        }
    }

    rets
}

/// Grab all layouts for the provided state, mapped to package id
fn layouts_for_state(client: &Client, state: &State) -> Result<Vec<(Id, Layout)>, db::Error> {
    client.layout_db.query(state.selections.iter().map(|s| &s.package))
}

/// Return an additional 4 older states excluding the current state
fn states_except_new(client: &Client, state: &State) -> Result<Vec<State>, db::Error> {
    let states = client
        .state_db
        .list_ids()?
        .into_iter()
        .filter_map(|(id, whence)| {
            // All states with older ID and not the current state
            if id != state.id && state.id > id {
                Some((id, whence))
            } else {
                None
            }
        })
        .sorted_by_key(|(_, whence)| whence.to_owned())
        .rev()
        .take(4)
        .filter_map(|(id, _)| client.state_db.get(id).ok())
        .collect::<Vec<_>>();
    Ok(states)
}

pub fn synchronize(client: &Client, state: &State) -> Result<(), Error> {
    let root = client.installation.root.clone();
    let is_native = root.to_string_lossy() == "/";
    // Create an appropriate configuration
    let config = blsforme::Configuration {
        root: if is_native {
            blsforme::Root::Native(root.clone())
        } else {
            blsforme::Root::Image(root.clone())
        },
        vfs: "/".into(),
    };

    // For the new/active state
    let head_layouts = layouts_for_state(client, state)?;
    let kernel_pattern = Pattern::from_str("lib/kernel/(version:*)/*")?;
    let systemd = Pattern::from_str("lib*/systemd/boot/efi/*.efi")?;
    let booty_bits = boot_files_from_new_state(&client.installation, &head_layouts, &systemd);

    let mut all_states = states_except_new(client, state)?;

    // no fun times without a bootloder
    if booty_bits.is_empty() {
        return Ok(());
    }

    // Read the os-release file we created
    // TODO: This needs per-state generation for the VERSION bits!
    let fp = fs::read_to_string(root.join("usr").join("lib").join("os-release"))?;
    let os_release = OsRelease::from_str(&fp)?;
    let schema = Schema::Blsforme {
        os_release: &os_release,
    };

    // Grab the entries for the new state
    let mut all_kernels = vec![];
    all_states.insert(0, state.clone());
    for state in all_states.iter() {
        let layouts = layouts_for_state(client, state)?;
        let local_kernels = kernel_files_from_state(&layouts, &kernel_pattern);
        let mapped = schema.discover_system_kernels(local_kernels.into_iter())?;
        all_kernels.push((mapped, state.id));
    }

    // pipe all of our entries into blsforme
    let mut entries = all_kernels
        .iter()
        .flat_map(|(kernels, state_id)| {
            kernels
                .iter()
                .map(|k| {
                    Entry::new(k)
                        .with_cmdline(CmdlineEntry {
                            name: "---fstx---".to_owned(),
                            snippet: format!("moss.fstx={state_id}"),
                        })
                        .with_state_id(i32::from(*state_id))
                        .with_sysroot(if state.id == *state_id {
                            root.clone()
                        } else {
                            client.installation.root_path(state_id.to_string()).to_owned()
                        })
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for entry in entries.iter_mut() {
        if let Err(e) = entry.load_cmdline_snippets(&config) {
            log::warn!("Failed to load cmdline snippets: {}", e);
        }
    }
    // no usable entries, lets get out of here.
    if entries.is_empty() {
        return Ok(());
    }

    // If we can't get a manager, find, but don't bomb. Its probably a topology failure.
    let manager = match blsforme::Manager::new(&config) {
        Ok(m) => m.with_entries(entries.into_iter()).with_bootloader_assets(booty_bits),
        Err(_) => return Ok(()),
    };

    // Only allow mounting pre-sync for a native run
    if is_native {
        let _mounts = manager.mount_partitions()?;
        manager.sync(&schema)?;
    } else {
        manager.sync(&schema)?;
    }

    Ok(())
}
