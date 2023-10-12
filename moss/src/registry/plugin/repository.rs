// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use log::warn;

use crate::{
    db,
    package::{self, Package},
    registry::job::Job,
    repository, Dependency, Provider,
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
                meta: package::Meta {
                    // TODO: Is there a more type-safe way to do this vs mutation? Can
                    // a new type help here?
                    uri: meta
                        .uri
                        .and_then(|relative| self.active.repository.uri.join(&relative).ok())
                        .map(|url| url.to_string()),
                    ..meta
                },
                flags: package::Flags::AVAILABLE,
            }),
            Err(db::meta::Error::RowNotFound) => None,
            Err(error) => {
                warn!("failed to query repository package: {error}");
                None
            }
        }
    }

    async fn query(&self, flags: package::Flags, filter: Option<db::meta::Filter>) -> Vec<Package> {
        if flags.contains(package::Flags::AVAILABLE) || flags == package::Flags::NONE {
            // TODO: Error handling
            let packages = match self.active.db.query(filter).await {
                Ok(packages) => packages,
                Err(error) => {
                    warn!("failed to query repository packages: {error}");
                    return vec![];
                }
            };

            packages
                .into_iter()
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
        self.query(flags, None).await
    }

    /// Query all packages that match the given provider identity
    pub async fn query_provider(&self, provider: &Provider, flags: package::Flags) -> Vec<Package> {
        self.query(flags, Some(db::meta::Filter::Provider(provider.clone())))
            .await
    }

    /// Query all packages that match the given dependency
    pub async fn query_dependency(
        &self,
        dependency: &Dependency,
        flags: package::Flags,
    ) -> Vec<Package> {
        self.query(
            flags,
            Some(db::meta::Filter::Dependency(dependency.clone())),
        )
        .await
    }

    pub async fn query_name(
        &self,
        package_name: &package::Name,
        flags: package::Flags,
    ) -> Vec<Package> {
        self.query(flags, Some(db::meta::Filter::Name(package_name.clone())))
            .await
    }

    pub fn fetch_item(&self, id: &package::Id) -> Job {
        todo!()
    }
}

impl PartialEq for Repository {
    fn eq(&self, other: &Self) -> bool {
        self.active.id.eq(&other.active.id)
    }
}

impl Eq for Repository {}
