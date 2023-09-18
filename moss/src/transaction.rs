// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::package;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Id(u64);

/// A Transaction is used to modify one system state to another
#[derive(Default, Clone, Debug)]
pub struct Transaction {
    // Unique identifier - baked only for commited transactions
    id: Option<Id>,

    // Package set
    packages: Vec<package::Id>
}

impl Transaction {
    /// Add packages, resolving dependencies
    pub fn add(id: Vec<package::Id>) -> Result<(), Error> {
        Err(Error::NotImplemented)
    }

    /// Remove a set of packages and reverse dependencies
    pub fn remove(id: Vec<package::Id>) -> Result<(), Error> {
        Err(Error::NotImplemented)
    }

    /// Return the package IDs in the fully baked configuration
    pub fn finalize() -> Result<Vec<package::Id>, Error> {
        Err(Error::NotImplemented)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("not yet implemented")]
    NotImplemented,
}
