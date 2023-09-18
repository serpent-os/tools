// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use thiserror::Error;

use crate::{
    registry::plugin::{self, cobble},
    Installation, Registry,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Root is invalid")]
    RootInvalid,
}

/// A Client is a connection to the underlying package management systems
pub struct Client {
    /// Root that we operate on
    installation: Installation,
    registry: Registry,
}

impl Client {
    /// Construct a new Client
    pub fn new_for_root(root: impl Into<PathBuf>) -> Result<Client, Error> {
        let root = root.into();

        if !root.exists() || !root.is_dir() {
            return Err(Error::RootInvalid);
        }

        let installation = Installation::open(root);
        let mut registry = Registry::default();
        // TODO: Seed with plugins for the Installation

        let cobble = cobble::Plugin::default();
        registry.add_plugin(plugin::Plugin::Cobble(cobble));

        Ok(Client {
            installation,
            registry,
        })
    }

    /// Construct a new Client for the global installation
    pub fn system() -> Result<Client, Error> {
        Client::new_for_root("/")
    }

    /// Borrow the registry
    pub fn registry(&mut self) -> &Registry {
        &self.registry
    }
}

impl Drop for Client {
    // Automatically drop resources for the client
    fn drop(&mut self) {}
}
