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

mod active;
pub mod cobble;
mod repository;

/// A [`Registry`] plugin that enables querying [`Package`] information.
///
/// [`Registry`]: super::Registry
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plugin {
    Active(active::Plugin),
    Cobble(cobble::Plugin),
    Repository(repository::Plugin),

    #[cfg(test)]
    Test(test::Plugin),
}

impl Plugin {
    /// Return a package for the given [`package::Id`]. Returns `None` if
    /// the `package` cannot be located.
    pub fn package(&self, id: &package::Id) -> Option<Package> {
        match self {
            Plugin::Active(_) => None,
            Plugin::Cobble(plugin) => plugin.package(id),
            Plugin::Repository(_) => None,

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.package(id),
        }
    }

    /// List all packages with matching `flags`
    pub fn list(&self, flags: package::Flags) -> package::Sorted<Vec<Package>> {
        package::Sorted::new(match self {
            Plugin::Active(_) => vec![],
            Plugin::Cobble(plugin) => plugin.list(flags),
            Plugin::Repository(_) => vec![],

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.list(flags),
        })
    }

    /// Returns a list of packages with matching `provider` and `flags`
    pub fn query_provider(
        &self,
        provider: &Provider,
        flags: package::Flags,
    ) -> package::Sorted<Vec<Package>> {
        package::Sorted::new(match self {
            Plugin::Active(_) => vec![],
            Plugin::Cobble(plugin) => plugin.query_provider(provider, flags),
            Plugin::Repository(_) => vec![],

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.query_provider(provider, flags),
        })
    }

    /// Returns a list of packages with matching `package_name` and `flags`
    pub fn query_name(
        &self,
        package_name: &package::Name,
        flags: package::Flags,
    ) -> package::Sorted<Vec<Package>> {
        package::Sorted::new(match self {
            Plugin::Active(_) => vec![],
            Plugin::Cobble(plugin) => plugin.query_name(package_name, flags),
            Plugin::Repository(_) => vec![],

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.query_name(package_name, flags),
        })
    }

    /// Plugin priority
    ///
    /// Higher priority = better chance of selection
    pub fn priority(&self) -> u64 {
        match self {
            Plugin::Active(_) => todo!(),
            Plugin::Cobble(plugin) => plugin.priority(),
            Plugin::Repository(_) => todo!(),

            #[cfg(test)]
            Plugin::Test(plugin) => plugin.priority,
        }
    }

    // /// Request that the item is fetched from its location into a storage
    // /// medium.
    // pub fn fetch_item(&self, package: &package::Id) -> package::Job {
    //     todo!();
    // }
}

/// Defines a [`Plugin`] ordering based on "priority", sorted
/// highest to lowest
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PriorityOrdered(pub Plugin);

impl PartialOrd for PriorityOrdered {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityOrdered {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.priority().cmp(&other.0.priority()).reverse()
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Plugin {
        pub priority: u64,
        packages: Vec<Package>,
    }

    impl Plugin {
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
    }
}
