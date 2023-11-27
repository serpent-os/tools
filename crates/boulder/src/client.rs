// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fs::create_dir,
    future::Future,
    io,
    path::{Path, PathBuf},
};

use moss::repository;
use thiserror::Error;
use tokio::runtime;

use crate::{profile, Profile};

pub struct Client {
    pub config: config::Manager,
    pub cache_dir: PathBuf,
    pub moss_dir: PathBuf,
    pub profiles: profile::Map,

    runtime: tokio::runtime::Runtime,
}

impl Client {
    pub fn new(
        config_dir: Option<PathBuf>,
        cache_dir: Option<PathBuf>,
        moss_root: Option<PathBuf>,
    ) -> Result<Self, Error> {
        let is_root = is_root();

        let config = if let Some(dir) = config_dir {
            config::Manager::custom(dir)
        } else if is_root {
            config::Manager::system("/", "boulder")
        } else {
            config::Manager::user("boulder")?
        };

        let cache_dir = resolve_cache_dir(is_root, cache_dir)?;
        let moss_dir = resolve_moss_root(is_root, moss_root)?;

        ensure_dir_exists(&cache_dir)?;
        ensure_dir_exists(&moss_dir)?;

        let runtime = runtime::Builder::new_multi_thread().enable_all().build()?;

        let profiles = runtime
            .block_on(config.load::<profile::Map>())
            .unwrap_or_default();

        Ok(Self {
            config,
            cache_dir,
            moss_dir,
            profiles,
            runtime,
        })
    }

    pub fn repositories(&self, profile: &profile::Id) -> Result<&repository::Map, Error> {
        self.profiles
            .get(profile)
            .map(|profile| &profile.collections)
            .ok_or(Error::MissingProfile)
    }

    pub fn save_profile(&mut self, id: profile::Id, profile: Profile) -> Result<(), Error> {
        // Save config
        let map = profile::Map::with([(id.clone(), profile.clone())]);
        self.runtime.block_on(self.config.save(id.clone(), &map))?;

        // Add to profile map
        self.profiles.add(id, profile);

        Ok(())
    }

    pub fn block_on<T, F>(&self, f: F) -> T
    where
        F: Future<Output = T>,
    {
        self.runtime.block_on(f)
    }
}

fn is_root() -> bool {
    use nix::unistd::Uid;

    Uid::effective().is_root()
}

fn resolve_cache_dir(is_root: bool, custom: Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(dir) = custom {
        Ok(dir)
    } else if is_root {
        Ok(PathBuf::from("/var/cache/boulder"))
    } else {
        Ok(dirs::cache_dir().ok_or(Error::UserCache)?.join("boulder"))
    }
}

fn resolve_moss_root(is_root: bool, custom: Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(dir) = custom {
        Ok(dir)
    } else if is_root {
        Ok(PathBuf::from("/"))
    } else {
        Ok(dirs::cache_dir().ok_or(Error::UserCache)?.join("moss"))
    }
}

fn ensure_dir_exists(path: &Path) -> Result<(), Error> {
    if !path.exists() {
        create_dir(path)?;
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot find the provided profile")]
    MissingProfile,
    #[error("cannot find cache dir, $XDG_CACHE_HOME or $HOME env not set")]
    UserCache,
    #[error("cannot find config dir, $XDG_CONFIG_HOME or $HOME env not set")]
    UserConfig,
    #[error("save config")]
    SaveConfig(#[from] config::SaveError),
    #[error("io")]
    Io(#[from] io::Error),
}

impl From<config::CreateUserError> for Error {
    fn from(_: config::CreateUserError) -> Self {
        Error::UserConfig
    }
}
