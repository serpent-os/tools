// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use futures::future;
use thiserror::Error;
use tokio::{fs, task};

use crate::db::meta;
use crate::{config, Installation};

pub use self::repository::Repository;

pub mod repository;

pub struct Remote {
    installation: Installation,
    repositories: HashMap<repository::Id, State>,
}

impl Remote {
    pub async fn new(installation: Installation) -> Result<Self, Error> {
        // Load all configs, default if none exist
        let configs = config::load::<repository::Map>(&installation.root)
            .await
            .unwrap_or_default();

        // Open all repo meta dbs and collect into hash map
        let repositories =
            future::try_join_all(configs.into_iter().map(|(id, repository)| async {
                let db = open_meta_db(&id, &installation)
                    .await
                    .map_err(|error| Error::OpenDatabase(id.clone(), error))?;

                Ok((id, State { repository, db }))
            }))
            .await?
            .into_iter()
            .collect();

        Ok(Self {
            installation,
            repositories,
        })
    }

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
                .map_err(|save| Error::SaveConfig(id.clone(), save))?;
        }

        let db = open_meta_db(&id, &self.installation)
            .await
            .map_err(|error| Error::OpenDatabase(id.clone(), error))?;

        self.repositories.insert(id, State { repository, db });

        Ok(())
    }

    pub async fn refresh_all(&self) -> Result<(), Error> {
        // Fetch index file + add to meta_db
        future::try_join_all(self.repositories.iter().map(|(id, state)| async {
            refresh_index_file(id, state, &self.installation)
                .await
                .map_err(|error| Error::RefreshIndex(id.clone(), error))
        }))
        .await?;

        Ok(())
    }
}

struct State {
    repository: Repository,
    db: meta::Database,
}

async fn open_meta_db(
    id: &repository::Id,
    installation: &Installation,
) -> Result<meta::Database, Box<dyn std::error::Error>> {
    let dir = installation.remotes_path(id.to_string());

    fs::create_dir_all(&dir).await?;

    let db = meta::Database::new(dir.join("db"), installation.read_only()).await?;

    Ok(db)
}

async fn refresh_index_file(
    id: &repository::Id,
    state: &State,
    installation: &Installation,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = installation.remotes_path(id.to_string());

    fs::create_dir_all(&out_dir).await?;

    let out_path = out_dir.join("stone.index");

    // Fetch index & write to `out_path`
    repository::fetch_index(state.repository.url.clone(), &out_path).await?;

    // Take ownership, thread needs 'static / owned data
    let db = state.db.clone();

    // We have to read the stone file in a blocking context
    let tasks = task::spawn_blocking(move || {
        use std::fs::File;

        // TODO: Error handling
        let index_file = File::open(&out_path).unwrap();
        let mut reader = stone::read(index_file).unwrap();

        let mut handles = vec![];

        for payload in reader.payloads().unwrap() {
            // We only care about meta payloads for index files
            let Ok(stone::read::Payload::Meta(payload)) = payload else {
                continue;
            };

            // Take ownership, future needs 'static / owned data
            let db = db.clone();

            // db is async, spawn the task back on tokio runtime
            handles.push(task::spawn(async move {
                db.load_stone_metadata(&payload).await
            }));
        }

        // return all spawned db tasks
        handles
    })
    .await?;

    // Run all db tasks to completion
    future::try_join_all(tasks)
        .await?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to save repository config {0}: {1}")]
    SaveConfig(repository::Id, config::SaveError),
    #[error("failed to refresh index file for repository {0}: {1}")]
    RefreshIndex(repository::Id, Box<dyn std::error::Error>),
    #[error("couldn't open meta database for repository {0}: {1}")]
    OpenDatabase(repository::Id, Box<dyn std::error::Error>),
}
