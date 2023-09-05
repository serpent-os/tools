// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Root is invalid")]
    RootInvalid,
}

/// A Client is a connection to the underlying package management systems
pub struct Client {
    /// Root that we operate on
    root: PathBuf,
}

impl Client {
    /// Construct a new Client
    pub fn new_for_root(root: PathBuf) -> Result<Client, Error> {
        if !root.exists() || !root.is_dir() {
            Err(Error::RootInvalid)
        } else {
            Ok(Client { root })
        }
    }

    /// Construct a new Client for the global installation
    pub fn system() -> Result<Client, Error> {
        Client::new_for_root(PathBuf::from("/"))
    }
}

impl Drop for Client {
    // Automatically drop resources for the client
    fn drop(&mut self) {}
}
