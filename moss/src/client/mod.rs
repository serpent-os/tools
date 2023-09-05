// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

/// A Client is a connection to the underlying package management systems
pub struct Client {}

impl Client {
    /// Construct a new Client
    fn new() -> Client {
        Client {}
    }
}

impl Drop for Client {
    // Automatically drop resources for the client
    fn drop(&mut self) {
        todo!()
    }
}
