// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashSet;

use dag::{toposort, Dfs, DiGraph};
use futures::{StreamExt, TryFutureExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{package, Dependency, Provider, Registry};

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
        packages: HashSet::new(),
    })
}

impl<'a> Transaction<'a> {
    /// Add a package to this transaction
    pub async fn add(&mut self, incoming: Vec<package::Id>) -> Result<(), Error> {
        self.packages.extend(self.compute_deps(incoming).await?);
        Ok(())
    }

    /// Remove a set of packages and reverse dependencies
    pub async fn remove(&mut self, incoming: Vec<package::Id>) -> Result<(), Error> {
        let mut to_remove = incoming.clone();

        // Get all installed reverse deps
        for id in incoming {
            to_remove.extend(self.compute_installed_reverse_deps(id.clone()).await?);
        }

        // Remove incoming and reverse deps
        self.packages.retain(|p| !to_remove.contains(p));

        Ok(())
    }

    /// Return the package IDs in the fully baked configuration
    pub fn finalize(&self) -> HashSet<package::Id> {
        self.packages.iter().cloned().collect()
    }

    /// Return all of the dependencies for input ID
    async fn compute_deps(&self, incoming: Vec<package::Id>) -> Result<Vec<package::Id>, Error> {
        let mut graph = DiGraph::new();
        let mut items = incoming.clone();

        loop {
            if items.is_empty() {
                break;
            }
            let mut next = vec![];
            for check_id in items.iter() {
                // See if the node exists yet..
                let check_node = graph
                    .node_indices()
                    .find(|i| graph[*i] == *check_id)
                    .unwrap_or_else(|| graph.add_node(check_id.clone()));

                // Grab this package in question
                let matches = self.registry.by_id(check_id).collect::<Vec<_>>().await;
                let package = matches
                    .first()
                    .ok_or(Error::NoCandidate(check_id.clone().into()))?;
                for dependency in package.meta.dependencies.iter() {
                    let provider = Provider {
                        kind: dependency.kind.clone(),
                        name: dependency.name.clone(),
                    };

                    // Now get it resolved
                    let search = self.resolve_installation_provider(provider).await?;

                    // Grab dependency node
                    let mut need_search = false;
                    let dep_node = graph
                        .node_indices()
                        .find(|i| graph[*i] == search)
                        .unwrap_or_else(|| {
                            need_search = true;
                            graph.add_node(search.clone())
                        });

                    // No dag node for it previously
                    if need_search {
                        next.push(search.clone());
                    }

                    // Connect w/ edges if non cyclical
                    let mut dfs = Dfs::new(&graph, dep_node);
                    let mut add_edge = true;
                    while let Some(item) = dfs.next(&graph) {
                        if item == dep_node {
                            add_edge = false;
                            break;
                        }
                    }
                    if graph.find_edge_undirected(check_node, dep_node).is_none() && add_edge {
                        graph.add_edge(check_node, dep_node, 1);
                    }
                }
            }
            items = next;
        }

        // topologically sort, returning a mapped cylical error if necessary
        // TODO: Handle emission of the cyclical error better and the chain involved
        Ok(toposort(&graph, None)
            .map_err(|e| Error::Cyclical(graph[e.node_id()].clone()))?
            .into_iter()
            .map(|i| graph[i].clone())
            .collect())
    }

    async fn compute_installed_reverse_deps(
        &self,
        id: package::Id,
    ) -> Result<Vec<package::Id>, Error> {
        let mut graph = DiGraph::new();
        let mut items = vec![id];

        loop {
            if items.is_empty() {
                break;
            }
            let mut next = vec![];
            for check_id in items.iter() {
                // See if the node exists yet..
                let check_node = graph
                    .node_indices()
                    .find(|i| graph[*i] == *check_id)
                    .unwrap_or_else(|| graph.add_node(check_id.clone()));

                // Grab this package in question
                let matches = self.registry.by_id(check_id).collect::<Vec<_>>().await;
                let package = matches
                    .first()
                    .ok_or(Error::NoCandidate(check_id.clone().into()))?;
                for provider in package.meta.providers.iter() {
                    let dependency = Dependency {
                        kind: provider.kind.clone(),
                        name: provider.name.clone(),
                    };

                    // Now get it resolved
                    let mut dependents = self
                        .registry
                        .by_dependency(&dependency, package::Flags::INSTALLED)
                        .map(|p| p.id)
                        .boxed();

                    while let Some(dependent) = dependents.next().await {
                        // Grab dependency node
                        let mut need_search = false;
                        let dep_node = graph
                            .node_indices()
                            .find(|i| graph[*i] == dependent)
                            .unwrap_or_else(|| {
                                need_search = true;
                                graph.add_node(dependent.clone())
                            });

                        // No dag node for it previously
                        if need_search {
                            next.push(dependent.clone());
                        }

                        // Connect w/ edges if non cyclical
                        let mut dfs = Dfs::new(&graph, dep_node);
                        let mut add_edge = true;
                        while let Some(item) = dfs.next(&graph) {
                            if item == dep_node {
                                add_edge = false;
                                break;
                            }
                        }
                        if graph.find_edge_undirected(check_node, dep_node).is_none() && add_edge {
                            graph.add_edge(check_node, dep_node, 1);
                        }
                    }
                }
            }
            items = next;
        }

        // topologically sort, returning a mapped cylical error if necessary
        // TODO: Handle emission of the cyclical error better and the chain involved
        Ok(toposort(&graph, None)
            .map_err(|e| Error::Cyclical(graph[e.node_id()].clone()))?
            .into_iter()
            .map(|i| graph[i].clone())
            .collect())
    }

    /// Attempt to resolve the filterered provider
    async fn resolve_provider(&self, filter: ProviderFilter) -> Result<package::Id, Error> {
        match filter {
            ProviderFilter::All(provider) => self
                .registry
                .by_provider(&provider, package::Flags::AVAILABLE)
                .boxed()
                .next()
                .await
                .map(|p| p.id.clone())
                .ok_or(Error::NoCandidate(provider.to_string())),
            ProviderFilter::InstalledOnly(provider) => self
                .registry
                .by_provider(&provider, package::Flags::INSTALLED)
                .boxed()
                .next()
                .await
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
                .boxed()
                .next()
                .await
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

    #[error("cyclical dependencies")]
    Cyclical(package::Id),

    #[error("no such name: {0}")]
    NoCandidate(String),

    #[error("not yet implemented")]
    NotImplemented,
}
