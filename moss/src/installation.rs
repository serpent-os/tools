// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use log::{trace, warn};
use nix::unistd::{access, AccessFlags, Uid};

use crate::db;

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

#[derive(Debug, Clone)]
pub struct Installation {
    pub root: PathBuf,
    pub mutability: Mutability,
    pub active_state: Option<db::state::Id>,
}

impl Installation {
    pub fn open(root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = root.into();

        let active_state = read_state_id(&root);

        if let Some(id) = &active_state {
            trace!("Active State ID: {id}");
        } else {
            warn!("Unable to discover Active State ID");
        }

        let mutability = if Uid::effective().is_root() {
            Mutability::ReadWrite
        } else if access(&root, AccessFlags::W_OK).is_ok() {
            Mutability::ReadWrite
        } else {
            Mutability::ReadOnly
        };

        trace!("Mutability: {mutability}");
        trace!("Root dir: {root:?}");

        if matches!(mutability, Mutability::ReadWrite) {
            ensure_dirs_exist(&root);
        }

        Self {
            root,
            mutability,
            active_state,
        }
    }

    pub fn read_only(&self) -> bool {
        matches!(self.mutability, Mutability::ReadOnly)
    }

    fn moss_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root.join(".moss").join(path)
    }

    pub fn db_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.moss_path("db").join(path)
    }

    pub fn cache_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.moss_path("cache").join(path)
    }

    pub fn assets_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.moss_path("assets").join(path)
    }

    pub fn root_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.moss_path("root").join(path)
    }

    pub fn staging_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root_path("staging").join(path)
    }

    pub fn staging_dir(&self) -> PathBuf {
        self.root_path("staging")
    }
}

/// In older versions of moss, the `/usr` entry was a symlink
/// to an active state. In newer versions, the state is recorded
/// within the installation tree. (`/usr/.stateID`)
fn read_state_id(root: &PathBuf) -> Option<db::state::Id> {
    let usr_path = root.join("usr");
    let state_path = root.join("usr").join(".stateID");

    if let Some(id) = fs::read_to_string(&state_path)
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
    {
        return Some(db::state::Id::from(id));
    } else if let Ok(usr_target) = usr_path.read_link() {
        return read_legacy_state_id(&usr_target);
    }

    None
}

fn read_legacy_state_id(usr_target: &PathBuf) -> Option<db::state::Id> {
    if usr_target.ends_with("usr") {
        let parent = usr_target.parent()?;
        let base = parent.file_name()?;
        let id = base.to_str()?.parse::<i64>().ok()?;

        return Some(db::state::Id::from(id));
    }

    None
}

/// Ensures moss directories are created
fn ensure_dirs_exist(root: &PathBuf) {
    let moss = root.join(".moss");

    for path in [
        moss.join("db"),
        moss.join("cache"),
        moss.join("remotes"),
        moss.join("root").join("staging"),
    ] {
        let _ = fs::create_dir_all(path);
    }
}
