// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use log::warn;

use crate::{db, package, Package, Provider, State};

// TODO:
#[derive(Debug, Clone)]
pub struct Active {
    state: Option<State>,
    db: crate::db::meta::Database,
}

impl PartialEq for Active {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

impl Eq for Active {}

impl Active {
    /// Return a new Active plugin for the given state + install database
    pub fn new(state: Option<State>, db: crate::db::meta::Database) -> Self {
        Self { state, db }
    }

    /// Query the given package
    pub async fn package(&self, id: &package::Id) -> Option<Package> {
        if self.pkg_installed(id) {
            let result = self.db.get(id).await;

            match result {
                Ok(meta) => Some(Package {
                    id: id.clone(),
                    meta,
                    flags: package::Flags::INSTALLED,
                }),
                Err(db::meta::Error::RowNotFound) => None,
                Err(error) => {
                    warn!("failed to query installed package: {error}");
                    None
                }
            }
        } else {
            None
        }
    }

    /// Query, restricted to state
    async fn query(&self, flags: package::Flags, filter: Option<db::meta::Filter>) -> Vec<Package> {
        if flags.contains(package::Flags::INSTALLED) || flags == package::Flags::NONE {
            // TODO: Error handling
            let packages = match self.db.query(filter).await {
                Ok(packages) => packages,
                Err(error) => {
                    warn!("failed to query repository packages: {error}");
                    return vec![];
                }
            };

            packages
                .into_iter()
                .filter(|(id, _)| self.pkg_installed(id))
                .map(|(id, meta)| Package {
                    id,
                    meta,
                    flags: package::Flags::INSTALLED,
                })
                .collect()
        } else {
            vec![]
        }
    }

    /// List, restricted to state
    pub async fn list(&self, flags: package::Flags) -> Vec<Package> {
        self.query(flags, None).await
    }

    /// Query all packages that match the given provider identity
    pub async fn query_provider(&self, provider: &Provider, flags: package::Flags) -> Vec<Package> {
        self.query(flags, Some(db::meta::Filter::Provider(provider.clone())))
            .await
    }

    /// Query matching by name
    pub async fn query_name(
        &self,
        package_name: &package::Name,
        flags: package::Flags,
    ) -> Vec<Package> {
        self.query(flags, Some(db::meta::Filter::Name(package_name.clone())))
            .await
    }

    pub fn priority(&self) -> u64 {
        u64::MAX
    }

    fn pkg_installed(&self, id: &package::Id) -> bool {
        match &self.state {
            Some(st) => st.packages.contains(id),
            None => false,
        }
    }
}
