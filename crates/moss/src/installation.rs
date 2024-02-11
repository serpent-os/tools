// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use log::{trace, warn};
use nix::unistd::{access, AccessFlags, Uid};

use crate::state;

/// System mutability - do we have readwrite?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    /// We only have readonly access
    ReadOnly,
    /// We have read-write access
    ReadWrite,
}

impl fmt::Display for Mutability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mutability::ReadOnly => "read-only".fmt(f),
            Mutability::ReadWrite => "read-write".fmt(f),
        }
    }
}

/// An Installation is a general encapsulation pattern for a root filesystem
/// as seen from moss.
/// We're largely active in the mutability, path builders and the potential active
/// state identifier.
#[derive(Debug, Clone)]
pub struct Installation {
    /// Fully qualified rootfs path
    pub root: PathBuf,

    /// Do we have R/W access?
    pub mutability: Mutability,

    /// Detected currently active state (optional)
    pub active_state: Option<state::Id>,
}

impl Installation {
    /// Open a system root as an Installation type
    /// This will query the potential active state if found,
    /// and determine the mutability per the current user identity
    /// and ACL permissions.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = root.into();

        let active_state = read_state_id(&root);

        if let Some(id) = &active_state {
            trace!("Active State ID: {id}");
        } else {
            warn!("Unable to discover Active State ID");
        }

        // Make sure directories exist (silently fail if read-only)
        //
        // It's important we try this first in-case `root` needs to be created
        // as well, otherwise mutability will always be read-only
        // TODO: Should we instead fail if root doesn't exist?
        ensure_dirs_exist(&root);

        // Root? Always RW. Otherwise, check access for W
        let mutability = if Uid::effective().is_root() || access(&root, AccessFlags::W_OK).is_ok() {
            Mutability::ReadWrite
        } else {
            Mutability::ReadOnly
        };

        trace!("Mutability: {mutability}");
        trace!("Root dir: {root:?}");

        Self {
            root,
            mutability,
            active_state,
        }
    }

    /// Return true if we lack write access
    pub fn read_only(&self) -> bool {
        matches!(self.mutability, Mutability::ReadOnly)
    }

    // Helper to form paths
    fn moss_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root.join(".moss").join(path)
    }

    /// Build a database path relative to the moss root
    pub fn db_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.moss_path("db").join(path)
    }

    /// Build a cache path relative to the moss root
    pub fn cache_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.moss_path("cache").join(path)
    }

    /// Build an asset path relative to the moss root
    pub fn assets_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.moss_path("assets").join(path)
    }

    /// Build a repo path relative to the root
    pub fn repo_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.moss_path("repo").join(path)
    }

    /// Build a path relative to the moss system roots tree
    pub fn root_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.moss_path("root").join(path)
    }

    /// Build a staging path for in-progress system root transactions
    pub fn staging_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root_path("staging").join(path)
    }

    /// Return the staging directory itself
    pub fn staging_dir(&self) -> PathBuf {
        self.root_path("staging")
    }

    /// Return the container dir itself
    pub fn isolation_dir(&self) -> PathBuf {
        self.root_path("isolation")
    }

    /// Build a container path for isolated triggers
    pub fn isolation_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root_path("isolation").join(path)
    }
}

/// In older versions of moss, the `/usr` entry was a symlink
/// to an active state. In newer versions, the state is recorded
/// within the installation tree. (`/usr/.stateID`)
fn read_state_id(root: &Path) -> Option<state::Id> {
    let usr_path = root.join("usr");
    let state_path = root.join("usr").join(".stateID");

    if let Some(id) = fs::read_to_string(state_path)
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
    {
        return Some(state::Id::from(id));
    } else if let Ok(usr_target) = usr_path.read_link() {
        return read_legacy_state_id(&usr_target);
    }

    None
}

// Legacy `/usr` link support
fn read_legacy_state_id(usr_target: &Path) -> Option<state::Id> {
    if usr_target.ends_with("usr") {
        let parent = usr_target.parent()?;
        let base = parent.file_name()?;
        let id = base.to_str()?.parse::<i64>().ok()?;

        return Some(state::Id::from(id));
    }

    None
}

/// Ensures moss directories are created
fn ensure_dirs_exist(root: &Path) {
    let moss = root.join(".moss");

    for path in [
        moss.join("db"),
        moss.join("cache"),
        moss.join("assets"),
        moss.join("repo"),
        moss.join("root").join("staging"),
        moss.join("root").join("isolation"),
    ] {
        let _ = fs::create_dir_all(path);
    }
    ensure_cachedir_tag(&moss.join("cache"));
}

fn ensure_cachedir_tag(path: &Path) {
    let cachedir_tag = path.join("CACHEDIR.TAG");
    if !cachedir_tag.exists() {
        let _ = std::fs::write(
            cachedir_tag,
            br#"Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by moss.
# For information about cache directory tags see https://bford.info/cachedir/"#,
        );
    }
}
