// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Defines an encapsulation of "query plugins", including an interface
//! for managing and using them.

use std::collections::BTreeSet;

use crate::package::{self, Package};
use crate::Provider;

pub use self::plugin::Plugin;

pub mod plugin;

/// A registry is composed of multiple "query plugins" that
/// provide [`Package`] information
#[derive(Debug, Default)]
pub struct Registry {
    /// Ordered set of plugins
    plugins: BTreeSet<plugin::PriorityOrdered>,
}

impl Registry {
    /// Add a [`Plugin`] to the [`Registry`]
    pub fn add_plugin(&mut self, plugin: Plugin) {
        self.plugins.insert(plugin::PriorityOrdered(plugin));
    }

    /// Remove a [`Plugin`] from the [`Registry`].
    pub fn remove_plugin(&mut self, plugin: &Plugin) {
        self.plugins.retain(|p| p.0 != *plugin);
    }

    /// Return a sorted iterator of [`Package`] by provider
    pub fn by_provider<'a: 'b, 'b>(
        &'a self,
        provider: &'b Provider,
        flags: package::Flags,
    ) -> impl Iterator<Item = Package> + 'b {
        // Returns an iterator of packages sorted by plugin priority (BTreeSet) then package ordering
        self.plugins
            .iter()
            .flat_map(move |p| p.0.query_provider(provider, flags).into_iter())
    }

    /// Return a sorted iterator of [`Package`] by name
    pub fn by_name<'a: 'b, 'b>(
        &'a self,
        package_name: &'b package::Name,
        flags: package::Flags,
    ) -> impl Iterator<Item = Package> + 'b {
        self.plugins
            .iter()
            .flat_map(move |p| p.0.query_name(package_name, flags).into_iter())
    }

    /// Return a sorted iterator of [`Package`] by id
    pub fn by_id<'a: 'b, 'b>(&'a self, id: &'b package::Id) -> impl Iterator<Item = Package> + 'b {
        self.plugins.iter().flat_map(|p| p.0.package(id))
    }

    /// Return a sorted iterator of [`Package`] matching the given [`Flags`]
    ///
    /// [`Flags`]: package::Flags
    pub fn list(&self, flags: package::Flags) -> impl Iterator<Item = Package> + '_ {
        self.plugins
            .iter()
            .flat_map(move |p| p.0.list(flags).into_iter())
    }

    /// Return a sorted iterator of installed [`Package`]
    pub fn list_installed(&self, flags: package::Flags) -> impl Iterator<Item = Package> + '_ {
        self.list(flags | package::Flags::INSTALLED)
    }

    /// Return a sorted iterator of available [`Package`]
    pub fn list_available(&self, flags: package::Flags) -> impl Iterator<Item = Package> + '_ {
        self.list(flags | package::Flags::AVAILABLE)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn test_ordering() {
        let mut registry = Registry::default();

        let package = |id: &str, release| Package {
            id: package::Id::from(id.to_string()),
            meta: package::Meta {
                name: package::Name::from(id.to_string()),
                version_identifier: Default::default(),
                source_release: release,
                build_release: Default::default(),
                architecture: Default::default(),
                summary: Default::default(),
                description: Default::default(),
                source_id: Default::default(),
                homepage: Default::default(),
                licenses: Default::default(),
                dependencies: Default::default(),
                providers: Default::default(),
                uri: Default::default(),
                hash: Default::default(),
                download_size: Default::default(),
            },
            flags: package::Flags::NONE,
        };

        registry.add_plugin(Plugin::Test(plugin::test::Plugin::new(
            // Priority
            1,
            // Id / release number
            vec![package("a", 0), package("b", 100)],
        )));

        registry.add_plugin(Plugin::Test(plugin::test::Plugin::new(
            50,
            vec![package("c", 50), package("d", 1)],
        )));

        // Packages are sorted by plugin priority, desc -> release number, desc
        for (idx, package) in registry.list(package::Flags::NONE).enumerate() {
            let id = |id: &str| package::Id::from(id.to_string());

            match idx {
                0 => assert_eq!(package.id, id("c")),
                1 => assert_eq!(package.id, id("d")),
                2 => assert_eq!(package.id, id("b")),
                3 => assert_eq!(package.id, id("a")),
                _ => {}
            }
        }
    }

    #[test]
    fn test_flags() {
        let mut registry = Registry::default();

        let package = |id: &str, flags| Package {
            id: package::Id::from(id.to_string()),
            meta: package::Meta {
                name: package::Name::from(id.to_string()),
                version_identifier: Default::default(),
                source_release: Default::default(),
                build_release: Default::default(),
                architecture: Default::default(),
                summary: Default::default(),
                description: Default::default(),
                source_id: Default::default(),
                homepage: Default::default(),
                licenses: Default::default(),
                dependencies: Default::default(),
                providers: Default::default(),
                uri: Default::default(),
                hash: Default::default(),
                download_size: Default::default(),
            },
            flags,
        };

        registry.add_plugin(Plugin::Test(plugin::test::Plugin::new(
            1,
            vec![
                package("a", package::Flags::INSTALLED),
                package("b", package::Flags::AVAILABLE),
                package("c", package::Flags::SOURCE),
                package("d", package::Flags::SOURCE | package::Flags::INSTALLED),
                package("e", package::Flags::SOURCE | package::Flags::AVAILABLE),
            ],
        )));

        let installed = registry.list_installed(package::Flags::NONE);
        let available = registry.list_available(package::Flags::NONE);
        let installed_source = registry.list_installed(package::Flags::SOURCE);
        let available_source = registry.list_available(package::Flags::SOURCE);

        fn matches(iter: impl Iterator<Item = Package>, expected: &[&'static str]) -> bool {
            let packages = iter
                .map(|p| String::from(p.meta.name))
                .collect::<HashSet<_>>();
            let expected = expected
                .iter()
                .map(|s| s.to_string())
                .collect::<HashSet<_>>();

            packages == expected
        }

        assert!(matches(installed, &["a", "d"]));
        assert!(matches(available, &["b", "e"]));
        assert!(matches(installed_source, &["d"]));
        assert!(matches(available_source, &["e"]));
    }
}
