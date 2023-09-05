// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{error::Error, fmt::Display, path::PathBuf};

#[derive(Debug)]
pub enum ClientError {
    RootInvalid,
}

impl Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootInvalid => write!(f, "Root is invalid)"),
        }
    }
}

impl Error for ClientError {}

/// A Client is a connection to the underlying package management systems
pub struct Client {
    /// Root that we operate on
    root: PathBuf,
}

impl Client {
    /// Construct a new Client
    pub fn new_for_root(root: PathBuf) -> Result<Client, ClientError> {
        if !root.exists() || !root.is_dir() {
            Err(ClientError::RootInvalid)
        } else {
            Ok(Client { root })
        }
    }

    /// Construct a new Client for the global installation
    pub fn system() -> Result<Client, ClientError> {
        Client::new_for_root(PathBuf::from("/"))
    }
}

impl Drop for Client {
    // Automatically drop resources for the client
    fn drop(&mut self) {}
}
