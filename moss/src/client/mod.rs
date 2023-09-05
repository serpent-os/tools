// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

/// A Client is a connection to the underlying package management systems
pub struct Client {
    /// Root that we operate on
    root: PathBuf,
}

impl Client {
    /// Construct a new Client
    fn new_for_root(root: PathBuf) -> Client {
        Client { root }
    }

    /// Construct a new Client for the global installation
    fn system() -> Client {
        Client::new_for_root(PathBuf::from("/"))
    }
}

impl Drop for Client {
    // Automatically drop resources for the client
    fn drop(&mut self) {
        todo!()
    }
}
