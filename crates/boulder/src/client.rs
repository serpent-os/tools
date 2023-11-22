// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    io,
    path::{Path, PathBuf},
};

use moss::repository;
use thiserror::Error;
use tokio::fs::create_dir;

use crate::profile;

pub struct Client {
    pub config: config::Manager,
    pub cache: PathBuf,
    pub moss: PathBuf,
    pub profiles: profile::Map,
}

impl Client {
    pub async fn new(
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

        let profiles = config.load::<profile::Map>().await.unwrap_or_default();

        let cache = resolve_cache_dir(is_root, cache_dir)?;
        let moss = resolve_moss_root(is_root, moss_root)?;

        ensure_dir_exists(&cache).await?;
        ensure_dir_exists(&moss).await?;

        Ok(Self {
            config,
            cache,
            moss,
            profiles,
        })
    }

    pub fn repositories(&self, profile: &profile::Id) -> Result<&repository::Map, Error> {
        self.profiles
            .get(profile)
            .map(|profile| &profile.collections)
            .ok_or(Error::MissingProfile)
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

async fn ensure_dir_exists(path: &Path) -> Result<(), Error> {
    if !path.exists() {
        create_dir(&path).await?;
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
    #[error("io")]
    Io(#[from] io::Error),
}

impl From<config::CreateUserError> for Error {
    fn from(_: config::CreateUserError) -> Self {
        Error::UserConfig
    }
}
