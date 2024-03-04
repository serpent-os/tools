// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use futures::{stream, StreamExt, TryFutureExt, TryStreamExt};
use itertools::Itertools;
use thiserror::Error;
use xxhash_rust::xxh3::xxh3_64;

use crate::db::meta;
use crate::{environment, runtime};
use crate::{package, Installation};

use crate::repository::{self, Repository};

enum Source {
    System(config::Manager),
    Explicit { identifier: String, repos: repository::Map },
}

impl Source {
    fn identifier(&self) -> &str {
        match self {
            Source::System(_) => environment::NAME,
            Source::Explicit { identifier, .. } => identifier,
        }
    }
}

/// Manage a bunch of repositories
pub struct Manager {
    source: Source,
    installation: Installation,
    repositories: HashMap<repository::Id, repository::Active>,
}

impl Manager {
    pub fn is_explicit(&self) -> bool {
        matches!(self.source, Source::Explicit { .. })
    }

    /// Create a [`Manager`] for the supplied [`Installation`] using system configurations
    pub fn system(config: config::Manager, installation: Installation) -> Result<Self, Error> {
        Self::new(Source::System(config), installation)
    }

    /// Create a [`Manager`] for the supplied [`Installation`] using the provided configurations
    ///
    /// [`Manager`] can't be used to `add` new repos in this mode
    pub fn explicit(
        identifier: impl ToString,
        repos: repository::Map,
        installation: Installation,
    ) -> Result<Self, Error> {
        Self::new(
            Source::Explicit {
                identifier: identifier.to_string(),
                repos,
            },
            installation,
        )
    }

    fn new(source: Source, installation: Installation) -> Result<Self, Error> {
        let configs = match &source {
            Source::System(config) =>
            // Load all configs, default if none exist
            {
                config
                    .load::<repository::Map>()
                    .into_iter()
                    .reduce(repository::Map::merge)
                    .unwrap_or_default()
            }
            Source::Explicit { repos, .. } => repos.clone(),
        };

        // Open all repo meta dbs and collect into hash map
        let repositories = configs
            .into_iter()
            .map(|(id, repository)| {
                let db = open_meta_db(source.identifier(), &repository, &installation)?;

                Ok((id.clone(), repository::Active { id, repository, db }))
            })
            .collect::<Result<_, Error>>()?;

        Ok(Self {
            source,
            installation,
            repositories,
        })
    }

    /// Add a [`Repository`]
    pub fn add_repository(&mut self, id: repository::Id, repository: Repository) -> Result<(), Error> {
        let Source::System(config) = &self.source else {
            return Err(Error::ExplicitUnsupported);
        };

        // Save repo as new config file
        // We save it as a map for easy merging across
        // multiple configuration files
        {
            let map = repository::Map::with([(id.clone(), repository.clone())]);
            config.save(&id, &map).map_err(Error::SaveConfig)?;
        }

        let db = open_meta_db(self.source.identifier(), &repository, &self.installation)?;

        self.repositories
            .insert(id.clone(), repository::Active { id, repository, db });

        Ok(())
    }

    /// Refresh all [`Repository`]'s by fetching it's latest index
    /// file and updating it's associated meta database
    pub async fn refresh_all(&mut self) -> Result<(), Error> {
        // Fetch index files asynchronously
        let fetched = stream::iter(&self.repositories)
            .map(|(_, state)| {
                fetch_index(self.source.identifier(), state, &self.installation)
                    .and_then(move |file| async move { Ok((state.clone(), file)) })
            })
            .buffer_unordered(environment::MAX_NETWORK_CONCURRENCY)
            .try_collect::<Vec<_>>()
            .await?;

        // Add each file to its meta_db
        for (state, file) in fetched {
            runtime::unblock(move || update_meta_db(&state, &file)).await?;
        }

        Ok(())
    }

    /// Refresh a [`Repository`] by Id
    pub async fn refresh(&mut self, id: &repository::Id) -> Result<(), Error> {
        if let Some(repo) = self.repositories.get(id).cloned() {
            let file = fetch_index(self.source.identifier(), &repo, &self.installation).await?;
            runtime::unblock(move || update_meta_db(&repo, &file)).await?;
            Ok(())
        } else {
            Err(Error::UnknownRepo(id.clone()))
        }
    }

    /// Ensures all repositories are initialized - index file downloaded and meta db
    /// populated.
    ///
    /// This is useful to call when initializing the moss client in-case users added configs
    /// manually outside the CLI
    pub async fn ensure_all_initialized(&mut self) -> Result<(), Error> {
        let uninitialized = self.repositories.iter().filter_map(|(id, state)| {
            let index_file =
                cache_dir(self.source.identifier(), &state.repository, &self.installation).join("stone.index");

            if !index_file.exists() {
                Some(state)
            } else {
                None
            }
        });

        let fetched = stream::iter(uninitialized)
            .map(|state| {
                fetch_index(self.source.identifier(), state, &self.installation)
                    .and_then(move |file| async move { Ok((state.clone(), file)) })
            })
            .buffer_unordered(environment::MAX_NETWORK_CONCURRENCY)
            .try_collect::<Vec<_>>()
            .await?;

        // Add each file to its meta_db
        for (state, file) in fetched {
            runtime::unblock(move || update_meta_db(&state, &file)).await?;
        }

        Ok(())
    }

    /// Returns the active repositories held by this manager
    pub(crate) fn active(&self) -> impl Iterator<Item = repository::Active> + '_ {
        self.repositories.values().cloned()
    }

    /// Remove a repository, deleting any related config & cached data
    pub fn remove(&mut self, id: impl Into<repository::Id>) -> Result<Removal, Error> {
        // Only allow removal for system repo manager
        let Source::System(config) = &self.source else {
            return Err(Error::ExplicitUnsupported);
        };

        // Remove from memory
        let Some(repo) = self.repositories.remove(&id.into()) else {
            return Ok(Removal::NotFound);
        };

        let cache_dir = cache_dir(self.source.identifier(), &repo.repository, &self.installation);

        // Remove cache
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir).map_err(Error::RemoveDir)?;
        }

        // Delete config, only succeeds for configs that live in their
        // own config file w/ matching repo name
        if config.delete::<repository::Map>(&repo.id).is_err() {
            return Ok(Removal::ConfigDeleted(false));
        }

        Ok(Removal::ConfigDeleted(true))
    }

    /// List all of the known repositories
    pub fn list(&self) -> impl ExactSizeIterator<Item = (&repository::Id, &Repository)> {
        self.repositories.iter().map(|(id, state)| (id, &state.repository))
    }
}

/// Directory for the repo cached data (db & stone index), hashed by identifier & repo URI
fn cache_dir(identifier: &str, repo: &Repository, installation: &Installation) -> PathBuf {
    let hash = format!("{:02x}", xxh3_64(format!("{}-{}", identifier, repo.uri).as_bytes()));
    installation.repo_path(hash)
}

/// Open the meta db file, ensuring it's
/// directory exists
fn open_meta_db(identifier: &str, repo: &Repository, installation: &Installation) -> Result<meta::Database, Error> {
    let dir = cache_dir(identifier, repo, installation);

    fs::create_dir_all(&dir).map_err(Error::CreateDir)?;

    let db = meta::Database::new(dir.join("db").to_str().unwrap_or_default())?;

    Ok(db)
}

/// Fetches a stone index file from the repository URL
/// and saves it to the repo installation path
async fn fetch_index(
    identifier: &str,
    state: &repository::Active,
    installation: &Installation,
) -> Result<PathBuf, Error> {
    let out_dir = cache_dir(identifier, &state.repository, installation);

    tokio::fs::create_dir_all(&out_dir).await.map_err(Error::CreateDir)?;

    let out_path = out_dir.join("stone.index");

    // Fetch index & write to `out_path`
    repository::fetch_index(state.repository.uri.clone(), &out_path).await?;

    Ok(out_path)
}

/// Updates a stones metadata into the meta db
fn update_meta_db(state: &repository::Active, index_path: &Path) -> Result<(), Error> {
    // Wipe db since we're refreshing from a new index file
    state.db.wipe()?;

    // Get a stream of payloads
    let mut file = File::open(index_path).map_err(Error::OpenIndex)?;
    let mut reader = stone::read(&mut file)?;
    let payloads = reader.payloads()?;

    // Update each payload into the meta db
    payloads
        // Batch up to `DB_BATCH_SIZE` payloads
        .chunks(environment::DB_BATCH_SIZE)
        .into_iter()
        // Transpose error for early bail
        .map(|chunk| chunk.into_iter().collect::<Result<Vec<_>, _>>())
        .try_for_each(|result| {
            let chunk = result?;

            // Construct Meta for each payload
            let packages = chunk
                .into_iter()
                .filter_map(|payload| {
                    if let stone::read::PayloadKind::Meta(meta) = payload {
                        Some(meta)
                    } else {
                        None
                    }
                })
                .map(|payload| {
                    let meta = package::Meta::from_stone_payload(&payload.body)?;

                    // Create id from hash of meta
                    let hash = meta
                        .hash
                        .clone()
                        .ok_or(Error::MissingMetaField(stone::payload::meta::Tag::PackageHash))?;
                    let id = package::Id::from(hash);

                    Ok((id, meta))
                })
                .collect::<Result<Vec<_>, Error>>()?;

            // Batch add to db
            //
            // Sqlite supports up to 32k parametized query binds. Adding a
            // package has 13 binds x 1k batch size = 17k. This leaves us
            // overhead to add more binds in the future, otherwise we can
            // lower the `DB_BATCH_SIZE`.
            state.db.batch_add(packages).map_err(Error::Database)
        })?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Can't modify repos when using explicit configs")]
    ExplicitUnsupported,
    #[error("Missing metadata field: {0:?}")]
    MissingMetaField(stone::payload::meta::Tag),
    #[error("create directory")]
    CreateDir(#[source] io::Error),
    #[error("remove directory")]
    RemoveDir(#[source] io::Error),
    #[error("fetch index file")]
    FetchIndex(#[from] repository::FetchError),
    #[error("open index file")]
    OpenIndex(#[source] io::Error),
    #[error("read index file")]
    ReadStone(#[from] stone::read::Error),
    #[error("meta db")]
    Database(#[from] meta::Error),
    #[error("save config")]
    SaveConfig(#[source] config::SaveError),
    #[error("unknown repo")]
    UnknownRepo(repository::Id),
}

impl From<package::MissingMetaFieldError> for Error {
    fn from(error: package::MissingMetaFieldError) -> Self {
        Self::MissingMetaField(error.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Removal {
    NotFound,
    ConfigDeleted(bool),
}
