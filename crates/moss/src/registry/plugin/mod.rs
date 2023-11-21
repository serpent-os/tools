// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Defines the notion of a registry plugin, which can be added to the
//! [`Registry`].
//!
//! Registry plugins are responsible for knowing how to talk to specific
//! backends hosting package info.
//!
//! [`Registry`]: super::Registry

use crate::registry::package::{self, Package};
use crate::Provider;

pub use self::active::Active;
pub use self::cobble::Cobble;
pub use self::repository::Repository;
#[cfg(test)]
pub use self::test::Test;

use super::job::Job;

mod active;
mod cobble;
mod repository;

/// A [`Registry`] plugin that enables querying [`Package`] information.
///
/// [`Registry`]: super::Registry
#[derive(Debug, PartialEq, Eq)]
pub enum Plugin {
    Active(Active),
    Cobble(Cobble),
    Repository(Repository),

    #[cfg(test)]
    Test(Test),
}

impl Plugin {
    /// Return a package for the given [`package::Id`]. Returns `None` if
    /// the `package` cannot be located.
    pub async fn package(&self, id: &package::Id) -> Option<Package> {
        match self {
            Plugin::Active(plugin) => plugin.package(id).await,
            Plugin::Cobble(plugin) => plugin.package(id),
            Plugin::Repository(plugin) => plugin.package(id).await,

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.package(id),
        }
    }

    /// List all packages with matching `flags`
    pub async fn list(&self, flags: package::Flags) -> package::Sorted<Vec<Package>> {
        package::Sorted::new(match self {
            Plugin::Active(plugin) => plugin.list(flags).await,
            Plugin::Cobble(plugin) => plugin.list(flags),
            Plugin::Repository(plugin) => plugin.list(flags).await,

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.list(flags),
        })
    }

    /// Returns a list of packages with matching `provider` and `flags`
    pub async fn query_provider(
        &self,
        provider: &Provider,
        flags: package::Flags,
    ) -> package::Sorted<Vec<Package>> {
        package::Sorted::new(match self {
            Plugin::Active(plugin) => plugin.query_provider(provider, flags).await,
            Plugin::Cobble(plugin) => plugin.query_provider(provider, flags),
            Plugin::Repository(plugin) => plugin.query_provider(provider, flags).await,

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.query_provider(provider, flags),
        })
    }

    /// Returns a list of packages with matching `package_name` and `flags`
    pub async fn query_name(
        &self,
        package_name: &package::Name,
        flags: package::Flags,
    ) -> package::Sorted<Vec<Package>> {
        package::Sorted::new(match self {
            Plugin::Active(plugin) => plugin.query_name(package_name, flags).await,
            Plugin::Cobble(plugin) => plugin.query_name(package_name, flags),
            Plugin::Repository(plugin) => plugin.query_name(package_name, flags).await,

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.query_name(package_name, flags),
        })
    }

    /// Plugin priority
    ///
    /// Higher priority = better chance of selection
    pub fn priority(&self) -> u64 {
        match self {
            Plugin::Active(plugin) => plugin.priority(),
            Plugin::Cobble(plugin) => plugin.priority(),
            Plugin::Repository(plugin) => plugin.priority(),

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.priority,
        }
    }

    /// Request that the item is fetched from its location into a storage
    /// medium.
    pub fn fetch_item(&self, id: &package::Id) -> Job {
        match self {
            Plugin::Active(_) => panic!("Active plugin queried for fetch"),
            Plugin::Cobble(plugin) => plugin.fetch_item(id),
            Plugin::Repository(plugin) => plugin.fetch_item(id),

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.fetch_item(id),
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::path::PathBuf;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Test {
        pub priority: u64,
        packages: Vec<Package>,
    }

    impl Test {
        pub fn new(priority: u64, packages: Vec<Package>) -> Self {
            Self { priority, packages }
        }

        pub fn package(&self, package: &package::Id) -> Option<Package> {
            self.packages.iter().find(|p| p.id == *package).cloned()
        }

        pub fn list(&self, flags: package::Flags) -> Vec<Package> {
            self.packages
                .iter()
                .filter(|p| p.flags.contains(flags))
                .cloned()
                .collect()
        }

        pub fn query_provider(&self, provider: &Provider, flags: package::Flags) -> Vec<Package> {
            self.packages
                .iter()
                .filter(|p| p.meta.providers.contains(provider) && p.flags.contains(flags))
                .cloned()
                .collect()
        }

        pub fn query_name(
            &self,
            package_name: &package::Name,
            flags: package::Flags,
        ) -> Vec<Package> {
            self.packages
                .iter()
                .filter(|p| p.meta.name == *package_name && p.flags.contains(flags))
                .cloned()
                .collect()
        }

        pub fn fetch_item(&self, id: &package::Id) -> Job {
            Job {
                domain: crate::registry::job::Domain::Package(id.clone()),
                origin: crate::registry::job::Origin::LocalFile(PathBuf::from(
                    "test/bash-completion-2.11-1-1-x86_64.stone",
                )),
                check: None,
                size: 168864,
            }
        }
    }
}
