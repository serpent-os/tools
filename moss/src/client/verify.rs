// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::{BTreeSet, HashSet},
    fs, io,
    path::PathBuf,
};

use itertools::Itertools;
use stone::{payload::layout, write::digest};
use vfs::tree::BlitFile;

use crate::{
    client::{self, cache},
    package, runtime, state, Client,
};

pub fn verify(client: &Client) -> Result<(), client::Error> {
    // Get all installed layouts, this is our source of truth
    let layouts = client.layout_db.all()?;

    // Group by unique assets (hash)
    let unique_assets = layouts
        .into_iter()
        .filter_map(|(package, layout)| {
            if let layout::Entry::Regular(hash, file) = layout.entry {
                Some((format!("{hash:02x}"), (package, file)))
            } else {
                None
            }
        })
        .into_group_map();

    let mut issues = vec![];
    let mut hasher = digest::Hasher::new();

    // For each asset, ensure it exists in the content store and isn't corrupt (hash is correct)
    for (hash, meta) in unique_assets.into_iter().sorted_by_key(|(key, _)| key.clone()) {
        let path = cache::asset_path(&client.installation, &hash);

        let files = meta.iter().map(|(_, file)| file).collect::<BTreeSet<_>>();

        println!("Checking {hash} - {files:?}");

        if !path.exists() {
            eprintln!("{path:?} doesn't exist");
            issues.push(Issue::MissingAsset {
                hash,
                packages: meta.into_iter().map(|(package, _)| package).collect(),
            });
            continue;
        }

        hasher.reset();

        let mut digest_writer = digest::Writer::new(io::sink(), &mut hasher);
        let mut file = fs::File::open(&path)?;

        // Copy bytes to null sink so we don't
        // explode memory
        io::copy(&mut file, &mut digest_writer)?;

        let verified_hash = format!("{:02x}", hasher.digest128());

        if verified_hash != hash {
            eprintln!("{path:?} hash mismatch, expected {hash} got {verified_hash}");
            issues.push(Issue::CorruptAsset {
                layout_hash: hash,
                corrupt_hash: verified_hash,
                packages: meta.into_iter().map(|(package, _)| package).collect(),
            });
        }
    }

    // Get all states
    let states = client.state_db.all()?;

    // Check the VFS of each state exists properly on the FS
    for state in &states {
        let is_active = client.installation.active_state == Some(state.id);

        let vfs = client.vfs(state.selections.iter().map(|s| &s.package))?;

        let base = if is_active {
            client.installation.root.join("usr")
        } else {
            client.installation.root_path(state.id.to_string()).join("usr")
        };

        for file in vfs.iter() {
            let path = base.join(file.path().strip_prefix("/usr/").unwrap_or_default());

            println!("Checking {path:?}");

            // All symlinks for non-active states are broken
            // since they resolve to the active state path
            //
            // Use try_exists to ensure we only check if symlink
            // itself is missing
            match path.try_exists() {
                Ok(true) => {}
                Ok(false) if path.is_symlink() => {}
                _ => {
                    eprintln!("{path:?} from state {} does not exist", state.id);
                    issues.push(Issue::MissingVFSPath { path, state: state.id })
                }
            }
        }
    }

    if issues.is_empty() {
        println!("No issues found");
        return Ok(());
    }

    println!(
        "Found {} issue{}, fixing...",
        issues.len(),
        if issues.len() == 1 { "" } else { "s" }
    );

    // Calculate and resolve the unique set of packages with asset issues
    let issue_packages = client.resolve_packages(
        issues
            .iter()
            .filter_map(Issue::packages)
            .flatten()
            .collect::<HashSet<_>>(),
    )?;

    // We had some corrupt or missing assets, let's resolve that!
    if !issue_packages.is_empty() {
        // Remove all corrupt assets
        for corrupt_hash in issues.iter().filter_map(Issue::corrupt_hash) {
            let path = cache::asset_path(&client.installation, corrupt_hash);
            fs::remove_file(&path)?;
        }

        println!("\nReinstalling packages");

        // And re-cache all packages that comprise the corrupt / missing asset
        runtime::block_on(client.cache_packages(&issue_packages))?;
    }

    // Now we must fix any states that referenced these packages
    // or had their own VFS issues that require a reblit
    let issue_states = states
        .iter()
        .filter_map(|state| {
            state
                .selections
                .iter()
                .any(|s| issue_packages.iter().any(|p| p.id == s.package))
                .then_some(&state.id)
        })
        .chain(issues.iter().filter_map(Issue::state))
        .collect::<BTreeSet<_>>();

    println!("\nReblitting affected states");

    // Reblit each state
    for id in issue_states {
        let state = states
            .iter()
            .find(|s| s.id == *id)
            .expect("must come from states originally");

        let is_active = client.installation.active_state == Some(state.id);

        // Blits to staging dir
        let fstree = client.blit_root(state.selections.iter().map(|s| &s.package))?;

        if is_active {
            // Override install root with the newly blitted active state
            client.apply_stateful_blit(fstree, state, None)?;
        } else {
            // Use the staged blit as an ephereral target for the non-active state
            // then archive it to it's archive directory
            client::record_state_id(&client.installation.staging_dir(), state.id)?;
            client.apply_ephemeral_blit(fstree, &client.installation.staging_dir())?;

            // Remove the old archive state so the new blit can be archived
            fs::remove_dir_all(client.installation.root_path(state.id.to_string()))?;
            client.archive_state(state.id)?;
        }

        println!("State {} fixed", state.id);
    }

    println!("All issues resolved");

    Ok(())
}

#[allow(dead_code)]
#[derive(Debug)]
enum Issue {
    CorruptAsset {
        layout_hash: String,
        corrupt_hash: String,
        packages: HashSet<package::Id>,
    },
    MissingAsset {
        hash: String,
        packages: HashSet<package::Id>,
    },
    MissingVFSPath {
        path: PathBuf,
        state: state::Id,
    },
}

impl Issue {
    fn corrupt_hash(&self) -> Option<&str> {
        match self {
            Issue::CorruptAsset { layout_hash: hash, .. } => Some(hash),
            Issue::MissingAsset { .. } => None,
            Issue::MissingVFSPath { .. } => None,
        }
    }

    fn packages(&self) -> Option<&HashSet<package::Id>> {
        match self {
            Issue::CorruptAsset { packages, .. } | Issue::MissingAsset { packages, .. } => Some(packages),
            Issue::MissingVFSPath { .. } => None,
        }
    }

    fn state(&self) -> Option<&state::Id> {
        match self {
            Issue::CorruptAsset { .. } | Issue::MissingAsset { .. } => None,
            Issue::MissingVFSPath { state, .. } => Some(state),
        }
    }
}
