// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use thiserror::Error;

use crate::{
    registry::plugin::{self, Plugin},
    repository, Installation, Registry,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Root is invalid")]
    RootInvalid,
    #[error("repository: {0}")]
    Repository(#[from] repository::manager::Error),
}

/// A Client is a connection to the underlying package management systems
pub struct Client {
    /// Root that we operate on
    pub installation: Installation,
    repositories: repository::Manager,
    pub registry: Registry,
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

        let registry = build_registry(&repositories);

        Ok(Client {
            installation,
            repositories,
            registry,
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

        // Rebuild registry
        self.registry = build_registry(&self.repositories);

        Ok(())
    }
}

fn build_registry(repositories: &repository::Manager) -> Registry {
    let mut registry = Registry::default();

    registry.add_plugin(Plugin::Cobble(plugin::Cobble::default()));

    for repo in repositories.active() {
        registry.add_plugin(Plugin::Repository(plugin::Repository::new(repo)));
    }

    registry
}

impl Drop for Client {
    // Automatically drop resources for the client
    fn drop(&mut self) {}
}
