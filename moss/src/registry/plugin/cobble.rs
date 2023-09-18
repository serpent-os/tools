// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::HashMap, path::PathBuf};

use crate::package::{self, meta, Meta, MissingMetaError, Package};
use crate::{stone, Provider};
use ::stone::read::Payload;
use futures::StreamExt;
use thiserror::Error;

// TODO:
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    // Storage of local packages
    packages: HashMap<meta::Id, State>,
}

impl Plugin {
    /// Add a package to the cobble set
    pub async fn add_package(&mut self, path: impl Into<PathBuf>) -> Result<(), Error> {
        let path = path.into();
        let (_, payloads) = stone::stream_payloads(&path).await?;

        // Grab the metapayload
        let metadata = payloads
            .filter_map(|result| async {
                if let Ok(Payload::Meta(meta)) = result {
                    Some(meta)
                } else {
                    None
                }
            })
            .boxed()
            .next()
            .await
            .ok_or(Error::MissingMetaPayload)?;

        // Whack it into the cobbler
        let meta = Meta::from_stone_payload(&metadata)?;
        let id = meta.id();

        self.packages.insert(id, State { path, meta });

        Ok(())
    }

    pub fn package(&self, id: &package::Id) -> Option<Package> {
        let meta_id = meta::Id::from(id.clone());

        self.packages
            .get(&meta_id)
            .map(|state| state.package(id.clone()))
    }

    fn query(&self, flags: package::Flags, filter: impl Fn(&Meta) -> bool) -> Vec<Package> {
        if flags.contains(package::Flags::AVAILABLE) {
            self.packages
                .iter()
                .filter(|(_, state)| filter(&state.meta))
                .map(|(id, state)| state.package(package::Id::from(id.clone())))
                .collect()
        } else {
            vec![]
        }
    }

    pub fn list(&self, flags: package::Flags) -> Vec<Package> {
        self.query(flags, |_| true)
    }

    pub fn query_provider(&self, provider: &Provider, flags: package::Flags) -> Vec<Package> {
        self.query(flags, |meta| meta.providers.contains(provider))
    }

    pub fn query_name(&self, package_name: &package::Name, flags: package::Flags) -> Vec<Package> {
        self.query(flags, |meta| meta.name == *package_name)
    }

    pub fn priority(&self) -> u64 {
        u64::MAX
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    path: PathBuf,
    meta: Meta,
}

impl State {
    fn package(&self, id: package::Id) -> Package {
        Package {
            id,
            meta: self.meta.clone(),
            // TODO: Is this correct flag?
            flags: package::Flags::AVAILABLE,
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] stone::read::Error),

    #[error("metadata: {0}")]
    Metadata(#[from] MissingMetaError),

    #[error("missing metadata payload")]
    MissingMetaPayload,

    #[error("unspecified error")]
    Unspecified,
}
