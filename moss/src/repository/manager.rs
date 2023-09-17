// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use futures::{future, TryStreamExt};
use itertools::Itertools;
use thiserror::Error;
use tokio::{fs, io};

use crate::db::meta;
use crate::stone;
use crate::{config, package, Installation};

use crate::repository::{self, Repository};

/// Manage a bunch of repositories
pub struct Manager {
    installation: Installation,
    repositories: HashMap<repository::Id, State>,
}

impl Manager {
    /// Create a [`Manager`] for the supplied [`Installation`]
    pub async fn new(installation: Installation) -> Result<Self, Error> {
        // Load all configs, default if none exist
        let configs = config::load::<repository::Map>(&installation.root)
            .await
            .unwrap_or_default();

        // Open all repo meta dbs and collect into hash map
        let repositories =
            future::try_join_all(configs.into_iter().map(|(id, repository)| async {
                let db = open_meta_db(&id, &installation).await?;

                Ok::<_, Error>((id, State { repository, db }))
            }))
            .await?
            .into_iter()
            .collect();

        Ok(Self {
            installation,
            repositories,
        })
    }

    /// Add a [`Repository`]
    pub async fn add_repository(
        &mut self,
        id: repository::Id,
        repository: Repository,
    ) -> Result<(), Error> {
        // Save repo as new config file
        // We save it as a map for easy merging across
        // multiple configuration files
        {
            let map = repository::Map::with([(id.clone(), repository.clone())]);

            config::save(&self.installation.root, &id, &map)
                .await
                .map_err(Error::SaveConfig)?;
        }

        let db = open_meta_db(&id, &self.installation).await?;

        self.repositories.insert(id, State { repository, db });

        Ok(())
    }

    /// Remove a [`Repository`]
    pub async fn remove_repository(&mut self, id: repository::Id) -> Result<(), Error> {
        self.repositories.remove(&id);

        let path = self.installation.repo_path(id.to_string());

        fs::remove_dir_all(path).await.map_err(Error::RemoveDir)?;

        Ok(())
    }

    /// Refresh all [`Repository`]'s by fetching it's latest index
    /// file and updating it's associated meta database
    pub async fn refresh_all(&mut self) -> Result<(), Error> {
        // Fetch index file + add to meta_db
        future::try_join_all(
            self.repositories
                .iter()
                .map(|(id, state)| refresh_index(id, state, &self.installation)),
        )
        .await?;

        Ok(())
    }

    /// Get access to the [`meta::Database`] of the managed repository
    pub(crate) fn get_meta_db(&self, id: &repository::Id) -> Option<&meta::Database> {
        self.repositories.get(id).map(|state| &state.db)
    }

    /// List all of the known repositories
    pub fn list(&self) -> Vec<(repository::Id, repository::Repository)> {
        self.repositories
            .iter()
            .map(|(id, state)| (id.clone(), state.repository.clone()))
            .collect_vec()
    }
}

struct State {
    repository: Repository,
    db: meta::Database,
}

/// Open the meta db file, ensuring it's
/// directory exists
async fn open_meta_db(
    id: &repository::Id,
    installation: &Installation,
) -> Result<meta::Database, Error> {
    let dir = installation.repo_path(id.to_string());

    fs::create_dir_all(&dir).await.map_err(Error::CreateDir)?;

    let db = meta::Database::new(dir.join("db"), installation.read_only()).await?;

    Ok(db)
}

/// Fetches a stone index file from the repository URL,
/// saves it to the repo installation path, then
/// loads it's metadata into the meta db
async fn refresh_index(
    id: &repository::Id,
    state: &State,
    installation: &Installation,
) -> Result<(), Error> {
    let out_dir = installation.repo_path(id.to_string());

    fs::create_dir_all(&out_dir)
        .await
        .map_err(Error::CreateDir)?;

    let out_path = out_dir.join("stone.index");

    // Fetch index & write to `out_path`
    repository::fetch_index(state.repository.uri.clone(), &out_path).await?;

    // Wipe db since we're refreshing from a new index file
    state.db.wipe().await?;

    // Get a stream of payloads
    let (_, payloads) = stone::stream_payloads(&out_path).await?;

    // Update each payload into the meta db
    payloads
        .map_err(Error::ReadStone)
        .try_for_each(|payload| async {
            // We only care about meta payloads for index files
            let stone::read::Payload::Meta(payload) = payload else {
                return Ok(());
            };

            let meta = package::Meta::from_stone_payload(&payload)?;

            // Create id from hash of meta
            let hash = meta.hash.clone().ok_or(Error::MissingMetaField(
                stone::payload::meta::Tag::PackageHash,
            ))?;
            let id = package::Id::from(hash);

            // Update db
            state.db.add(id, meta).await?;

            Ok(())
        })
        .await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to create directory: {0}")]
    CreateDir(io::Error),
    #[error("failed to remove directory: {0}")]
    RemoveDir(io::Error),
    #[error("failed to fetch index file: {0}")]
    FetchIndex(#[from] repository::FetchError),
    #[error("failed to read index file: {0}")]
    ReadStone(#[from] stone::read::Error),
    #[error("meta database error: {0}")]
    Database(#[from] meta::Error),
    #[error("failed to save config: {0}")]
    SaveConfig(config::SaveError),
    #[error("missing metadata field: {0:?}")]
    MissingMetaField(stone::payload::meta::Tag),
}

impl From<package::MissingMetaError> for Error {
    fn from(error: package::MissingMetaError) -> Self {
        Self::MissingMetaField(error.0)
    }
}
