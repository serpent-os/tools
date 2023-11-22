// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use thiserror::Error;

use crate::profile;

pub struct Client {
    pub config: config::Manager,
    pub cache: PathBuf,
    pub profiles: profile::Map,
}

impl Client {
    pub async fn new(
        config_dir: Option<PathBuf>,
        cache_dir: Option<PathBuf>,
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

        Ok(Self {
            config,
            cache,
            profiles,
        })
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot find cache dir, $XDG_CACHE_HOME or $HOME env not set")]
    UserCache,
    #[error("cannot find config dir, $XDG_CONFIG_HOME or $HOME env not set")]
    UserConfig,
}

impl From<config::CreateUserError> for Error {
    fn from(_: config::CreateUserError) -> Self {
        Error::UserConfig
    }
}
