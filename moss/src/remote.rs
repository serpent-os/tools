// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;
use std::path::PathBuf;

use futures::future;
use thiserror::Error;
use tokio::task::JoinHandle;
use tokio::{fs, io, task};

use crate::db::meta;
use crate::{config, Installation};

pub use self::repository::Repository;

pub mod repository;

/// Manage a bunch of remote repositories
pub struct Remote {
    installation: Installation,
    repositories: HashMap<repository::Id, State>,
}

impl Remote {
    /// Create a [`Remote`] for the supplied [`Installation`]
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

        let path = self.installation.remotes_path(id.to_string());

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
    let dir = installation.remotes_path(id.to_string());

    fs::create_dir_all(&dir).await.map_err(Error::CreateDir)?;

    let db = meta::Database::new(dir.join("db"), installation.read_only()).await?;

    Ok(db)
}

/// Fetches a stone index file from the repository URL,
/// saves it to the remote installation path, then
/// loads it's metadata into the meta db
async fn refresh_index(
    id: &repository::Id,
    state: &State,
    installation: &Installation,
) -> Result<(), Error> {
    let out_dir = installation.remotes_path(id.to_string());

    fs::create_dir_all(&out_dir)
        .await
        .map_err(Error::CreateDir)?;

    let out_path = out_dir.join("stone.index");

    // Fetch index & write to `out_path`
    repository::fetch_index(state.repository.url.clone(), &out_path).await?;

    // Wipe db since we're refreshing from a new index file
    state.db.wipe().await?;

    // Take ownership, thread needs 'static / owned data
    let db = state.db.clone();

    let tasks = task::spawn_blocking(move || update_meta_db(out_path, db))
        .await
        .expect("join handle")?;

    // Run all db tasks to completion
    future::try_join_all(tasks)
        .await
        .expect("join handle")
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}

/// Reads stone index file from `path` and streams each
/// index payload into the provided `db`
///
/// This function is blocking for reading the stone file, but spawns
/// an async task for each read payload for updating `db`.
///
/// This returns a vec of the handles to those async tasks, which can
/// be awaited on to ensure all db updates are processed
fn update_meta_db(
    path: PathBuf,
    db: meta::Database,
) -> Result<Vec<JoinHandle<Result<meta::Entry, meta::Error>>>, Error> {
    use std::fs::File;

    // Open file and read it
    let index_file = File::open(path).map_err(Error::OpenIndex)?;
    let mut reader = stone::read(index_file)?;

    // async db task handles
    let mut handles = vec![];

    for payload in reader.payloads()? {
        // We only care about meta payloads for index files
        let Ok(stone::read::Payload::Meta(payload)) = payload else {
                continue;
            };

        // Take ownership, future needs 'static / owned data
        let db = db.clone();

        // db is async, spawn task back on tokio runtime
        handles.push(task::spawn(async move {
            db.load_stone_metadata(&payload).await
        }));
    }

    // return all db tasks
    Ok(handles)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to create directory: {0}")]
    CreateDir(io::Error),
    #[error("failed to remove directory: {0}")]
    RemoveDir(io::Error),
    #[error("failed to fetch index file: {0}")]
    FetchIndex(#[from] repository::FetchError),
    #[error("failed to open index file: {0}")]
    OpenIndex(io::Error),
    #[error("failed to read index file: {0}")]
    ReadStone(#[from] stone::read::Error),
    #[error("meta database error: {0}")]
    Database(#[from] meta::Error),
    #[error("failed to save config: {0}")]
    SaveConfig(config::SaveError),
}
