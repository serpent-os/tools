// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use dag::Dag;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{package, Provider, Registry};

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

enum Lookup {
    InstalledOnly,
    Global,
}

/// A Transaction is used to modify one system state to another
#[derive(Clone, Debug)]
pub struct Transaction<'a> {
    // Unique identifier - baked only for commited transactions
    id: Option<Id>,

    // Bound to a registry
    registry: &'a Registry,

    // unique set of package ids
    packages: Dag<package::Id>,
}

/// Construct a new Transaction wrapped around the underlying Registry
/// At this point the registry is initialised and we can probe the installed
/// set.
pub(super) fn new(registry: &Registry) -> Result<Transaction<'_>, Error> {
    Ok(Transaction {
        id: None,
        registry,
        packages: Dag::default(),
    })
}

/// Populate the transaction on initialisation
pub(super) fn new_with_installed(registry: &Registry, incoming: Vec<package::Id>) -> Result<Transaction<'_>, Error> {
    let mut tx = new(registry)?;
    tx.update(incoming, Lookup::InstalledOnly)?;
    Ok(tx)
}

impl<'a> Transaction<'a> {
    /// Add a package to this transaction
    pub fn add(&mut self, incoming: Vec<package::Id>) -> Result<(), Error> {
        self.update(incoming, Lookup::Global)
    }

    /// Remove a set of packages and their reverse dependencies
    pub fn remove(&mut self, packages: Vec<package::Id>) {
        // Get transposed subgraph
        let transposed = self.packages.transpose();
        let subgraph = transposed.subgraph(&packages);

        // For each node, remove it from transaction graph
        subgraph.iter_nodes().for_each(|package| {
            // Remove that package
            self.packages.remove_node(package);
        });
    }

    /// Return the package IDs in the fully baked configuration
    pub fn finalize(&self) -> impl Iterator<Item = &package::Id> + '_ {
        self.packages.topo()
    }

    /// Update internal package graph with all incoming packages & their deps
    fn update(&mut self, incoming: Vec<package::Id>, lookup: Lookup) -> Result<(), Error> {
        let mut items = incoming;

        loop {
            if items.is_empty() {
                break;
            }
            let mut next = vec![];
            for check_id in items.iter() {
                // Ensure node is added and get it's index
                let check_node = self.packages.add_node_or_get_index(check_id.clone());

                // Grab this package in question
                let package = self
                    .registry
                    .by_id(check_id)
                    .next()
                    .ok_or(Error::NoCandidate(check_id.clone().into()))?;
                for dependency in package.meta.dependencies.iter() {
                    let provider = Provider {
                        kind: dependency.kind,
                        name: dependency.name.clone(),
                    };

                    // Now get it resolved
                    let search = match lookup {
                        Lookup::Global => self.resolve_installation_provider(provider)?,
                        Lookup::InstalledOnly => self.resolve_provider(ProviderFilter::InstalledOnly(provider))?,
                    };

                    // Add dependency node
                    let need_search = !self.packages.node_exists(&search);
                    let dep_node = self.packages.add_node_or_get_index(search.clone());

                    // No dag node for it previously
                    if need_search {
                        next.push(search.clone());
                    }

                    // Connect w/ edges (rejects cyclical & duplicate edges)
                    self.packages.add_edge(check_node, dep_node);
                }
            }
            items = next;
        }

        Ok(())
    }

    /// Attempt to resolve the filterered provider
    fn resolve_provider(&self, filter: ProviderFilter) -> Result<package::Id, Error> {
        match filter {
            ProviderFilter::All(provider) => self
                .registry
                .by_provider(&provider, package::Flags::new().with_available())
                .next()
                .map(|p| p.id)
                .ok_or(Error::NoCandidate(provider.to_string())),
            ProviderFilter::InstalledOnly(provider) => self
                .registry
                .by_provider(&provider, package::Flags::new().with_installed())
                .next()
                .map(|p| p.id)
                .ok_or(Error::NoCandidate(provider.to_string())),
            ProviderFilter::Selections(provider) => self
                .registry
                .by_provider(&provider, package::Flags::default())
                .find_map(|p| {
                    if self.packages.node_exists(&p.id) {
                        Some(p.id)
                    } else {
                        None
                    }
                })
                .ok_or(Error::NoCandidate(provider.to_string())),
        }
    }

    // Try all strategies to resolve a provider for installation
    fn resolve_installation_provider(&self, provider: Provider) -> Result<package::Id, Error> {
        self.resolve_provider(ProviderFilter::Selections(provider.clone()))
            .or_else(|_| self.resolve_provider(ProviderFilter::InstalledOnly(provider.clone())))
            .or_else(|_| self.resolve_provider(ProviderFilter::All(provider)))
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("No such name: {0}")]
    NoCandidate(String),

    #[error("Not yet implemented")]
    NotImplemented,

    #[error("meta db")]
    Database(#[from] crate::db::meta::Error),
}
