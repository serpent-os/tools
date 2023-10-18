// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use thiserror::Error;

use self::prune::prune;
use crate::{
    db,
    registry::plugin::{self, Plugin},
    repository, Installation, Registry, State,
};

pub mod prune;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Root is invalid")]
    RootInvalid,
    #[error("repository: {0}")]
    Repository(#[from] repository::manager::Error),
    #[error("meta: {0}")]
    Meta(#[from] db::meta::Error),

    #[error("layout: {0}")]
    Layout(#[from] db::layout::Error),
    #[error("state: {0}")]
    State(#[from] db::state::Error),

    #[error("prune: {0}")]
    Prune(#[from] prune::Error),
}

/// A Client is a connection to the underlying package management systems
pub struct Client {
    /// Root that we operate on
    pub installation: Installation,
    repositories: repository::Manager,
    pub registry: Registry,

    pub install_db: db::meta::Database,
    pub state_db: db::state::Database,
    pub layout_db: db::layout::Database,
}

impl Client {
    /// Construct a new Client
    pub async fn new_for_root(root: impl Into<PathBuf>) -> Result<Client, Error> {
        let root = root.into();

        if !root.exists() || !root.is_dir() {
            return Err(Error::RootInvalid);
        }

        let installation = Installation::open(root);
        let repositories = repository::Manager::new(installation.clone()).await?;
        let install_db =
            db::meta::Database::new(installation.db_path("install"), installation.read_only())
                .await?;
        let state_db = db::state::Database::new(&installation).await?;
        let layout_db = db::layout::Database::new(&installation).await?;

        let state = match installation.active_state {
            Some(id) => Some(state_db.get(&id).await?),
            None => None,
        };

        let registry = build_registry(&repositories, &install_db, state).await?;

        Ok(Client {
            installation,
            repositories,
            registry,
            install_db,
            state_db,
            layout_db,
        })
    }

    /// Construct a new Client for the global installation
    pub async fn system() -> Result<Client, Error> {
        Client::new_for_root("/").await
    }

    /// Reload all configured repositories and refreshes their index file, then update
    /// registry with all active repositories.
    pub async fn refresh_repositories(&mut self) -> Result<(), Error> {
        // Reload manager and refresh all repositories
        self.repositories = repository::Manager::new(self.installation.clone()).await?;
        self.repositories.refresh_all().await?;

        // Refresh State DB
        let state = match self.installation.active_state {
            Some(id) => Some(self.state_db.get(&id).await?),
            None => None,
        };

        // Rebuild registry
        self.registry = build_registry(&self.repositories, &self.install_db, state).await?;

        Ok(())
    }

    pub async fn prune(&self, strategy: prune::Strategy) -> Result<(), Error> {
        prune(strategy, &self.state_db, &self.install_db, &self.layout_db).await?;
        Ok(())
    }
}

async fn build_registry(
    repositories: &repository::Manager,
    installdb: &db::meta::Database,
    state: Option<State>,
) -> Result<Registry, Error> {
    let mut registry = Registry::default();

    registry.add_plugin(Plugin::Cobble(plugin::Cobble::default()));
    registry.add_plugin(Plugin::Active(plugin::Active::new(
        state,
        installdb.clone(),
    )));

    for repo in repositories.active() {
        registry.add_plugin(Plugin::Repository(plugin::Repository::new(repo)));
    }

    Ok(registry)
}

impl Drop for Client {
    // Automatically drop resources for the client
    fn drop(&mut self) {}
}
