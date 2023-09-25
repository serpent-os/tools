// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use futures::executor::block_on;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{db::meta::Database, package, Registry};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Id(u64);

/// A Transaction is used to modify one system state to another
#[derive(Clone, Debug)]
pub struct Transaction<'a> {
    // Unique identifier - baked only for commited transactions
    id: Option<Id>,

    // Bound to a registry
    registry: &'a Registry,

    /// Memory bound database for resolution
    db: Database,
}

/// Construct a new Transaction wrapped around the underlying Registry
/// At this point the registry is initialised and we can probe the installed
/// set.
pub(super) fn new(registry: &Registry) -> Result<Transaction<'_>, Error> {
    Ok(Transaction {
        id: None,
        registry,
        db: block_on(Database::new("sqlite::memory:", false))?,
    })
}

impl<'a> Transaction<'a> {
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
    #[error("database error: {0}")]
    Database(#[from] crate::db::meta::Error),

    #[error("not yet implemented")]
    NotImplemented,
}
