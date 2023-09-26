// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashSet;

use futures::{executor::block_on, StreamExt, TryFutureExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{db::meta::Database, package, Package, Provider, Registry};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Id(u64);

enum ProviderFilter {
    /// Must be installed
    InstalledOnly(Provider),

    /// Filter the lookup to current selection scope
    Selections(Provider),

    // Look beyond installed/selections
    All(Provider),
}

/// A Transaction is used to modify one system state to another
#[derive(Clone, Debug)]
pub struct Transaction<'a> {
    // Unique identifier - baked only for commited transactions
    id: Option<Id>,

    // Bound to a registry
    registry: &'a Registry,

    /// Memory bound database for resolution
    db: Database,

    // unique set of package ids
    packages: HashSet<package::Id>,
}

/// Construct a new Transaction wrapped around the underlying Registry
/// At this point the registry is initialised and we can probe the installed
/// set.
pub(super) fn new(registry: &Registry) -> Result<Transaction<'_>, Error> {
    Ok(Transaction {
        id: None,
        registry,
        db: block_on(Database::new("sqlite::memory:", false))?,
        packages: HashSet::new(),
    })
}

impl<'a> Transaction<'a> {
    /// Add a package to this transaction
    pub async fn add(&mut self, id: package::Id) -> Result<(), Error> {
        self.packages.extend(self.compute_deps(id).await?);
        Ok(())
    }

    /// Remove a set of packages and reverse dependencies
    pub fn remove(&self, id: package::Id) -> Result<(), Error> {
        Err(Error::NotImplemented)
    }

    /// Return the package IDs in the fully baked configuration
    pub fn finalize(&self) -> Result<Vec<package::Id>, Error> {
        Err(Error::NotImplemented)
    }

    /// Return all of the dependencies for input ID
    async fn compute_deps(&self, id: package::Id) -> Result<Vec<package::Id>, Error> {
        let lookup = self.registry.by_id(&id).collect::<Vec<_>>().await;
        let candidate = lookup.first().ok_or(Error::NoCandidate(id.into()))?;

        Ok(vec![candidate.id.clone()])
    }

    /// Attempt to resolve the filterered provider
    async fn resolve_provider(&self, filter: ProviderFilter) -> Result<package::Id, Error> {
        match filter {
            ProviderFilter::All(provider) => self
                .registry
                .by_provider(&provider, package::Flags::AVAILABLE)
                .collect::<Vec<_>>()
                .await
                .first()
                .map(|p| p.id.clone())
                .ok_or(Error::NoCandidate(provider.to_string())),
            ProviderFilter::InstalledOnly(provider) => self
                .registry
                .by_provider(&provider, package::Flags::INSTALLED)
                .collect::<Vec<_>>()
                .await
                .first()
                .map(|p| p.id.clone())
                .ok_or(Error::NoCandidate(provider.to_string())),
            ProviderFilter::Selections(provider) => self
                .registry
                .by_provider(&provider, package::Flags::NONE)
                .filter_map(|f| async {
                    if self.packages.contains(&f.id) {
                        Some(f)
                    } else {
                        None
                    }
                })
                .collect::<Vec<Package>>()
                .await
                .first()
                .map(|p| p.id.clone())
                .ok_or(Error::NoCandidate(provider.to_string())),
        }
    }

    // Try all strategies to resolve a provider for installation
    async fn resolve_installation_provider(
        &self,
        provider: Provider,
    ) -> Result<package::Id, Error> {
        self.resolve_provider(ProviderFilter::Selections(provider.clone()))
            .or_else(|_| async {
                self.resolve_provider(ProviderFilter::InstalledOnly(provider.clone()))
                    .await
            })
            .or_else(|_| async {
                self.resolve_provider(ProviderFilter::All(provider.clone()))
                    .await
            })
            .await
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(#[from] crate::db::meta::Error),

    #[error("no such name: {0}")]
    NoCandidate(String),

    #[error("not yet implemented")]
    NotImplemented,
}
