// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use log::warn;

use crate::{
    db,
    package::{self, Meta, Package},
    repository, Provider,
};

#[derive(Debug)]
pub struct Repository {
    active: repository::Active,
}

impl Repository {
    pub fn new(active: repository::Active) -> Self {
        Self { active }
    }

    pub fn priority(&self) -> u64 {
        self.active.repository.priority.into()
    }

    pub async fn package(&self, id: &package::Id) -> Option<Package> {
        let result = self.active.db.get(id).await;

        match result {
            Ok(meta) => Some(Package {
                id: id.clone(),
                meta,
                flags: package::Flags::AVAILABLE,
            }),
            Err(db::meta::Error::RowNotFound) => None,
            Err(error) => {
                warn!("failed to query repository package: {error}");
                None
            }
        }
    }

    async fn query(&self, flags: package::Flags, filter: impl Fn(&Meta) -> bool) -> Vec<Package> {
        if flags.contains(package::Flags::AVAILABLE) {
            // TODO: Error handling
            let packages = match self.active.db.all().await {
                Ok(packages) => packages,
                Err(error) => {
                    warn!("failed to query repository packages: {error}");
                    return vec![];
                }
            };

            packages
                .into_iter()
                .filter(|(_, meta)| filter(meta))
                .map(|(id, meta)| Package {
                    id,
                    meta,
                    flags: package::Flags::AVAILABLE,
                })
                .collect()
        } else {
            vec![]
        }
    }

    pub async fn list(&self, flags: package::Flags) -> Vec<Package> {
        self.query(flags, |_| true).await
    }

    /// Query all packages that match the given provider identity
    pub async fn query_provider(&self, provider: &Provider, flags: package::Flags) -> Vec<Package> {
        if !flags.contains(package::Flags::AVAILABLE) {
            return vec![];
        }

        let packages = self.active.db.get_providers(provider).await;
        if packages.is_err() {
            vec![]
        } else {
            packages
                .unwrap()
                .into_iter()
                .map(|(id, meta)| Package {
                    id,
                    meta,
                    flags: package::Flags::AVAILABLE,
                })
                .collect()
        }
    }

    pub async fn query_name(
        &self,
        package_name: &package::Name,
        flags: package::Flags,
    ) -> Vec<Package> {
        self.query(flags, |meta| meta.name == *package_name).await
    }
}

impl PartialEq for Repository {
    fn eq(&self, other: &Self) -> bool {
        self.active.id.eq(&other.active.id)
    }
}

impl Eq for Repository {}
