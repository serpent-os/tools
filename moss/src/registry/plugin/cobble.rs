// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::HashMap, path::PathBuf};

use crate::package::{Meta, MissingMetaError};
use crate::stone;
use ::stone::read::Payload;
use futures::StreamExt;
use thiserror::Error;

// TODO:
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    // Storage of local packages
    packages: HashMap<String, State>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    path: PathBuf,
    meta: Meta,
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
