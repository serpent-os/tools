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
    packages: HashMap<String, Meta>,
    id_to_path: HashMap<String, String>,
}

impl Plugin {
    /// Add a package to the cobble set
    pub async fn add_package(&mut self, path: impl Into<PathBuf>) -> Result<(), Error> {
        let path: PathBuf = path.into();
        let (_, mut payloads) = stone::stream_payloads(&path).await?;

        let mut metadata = vec![];

        // Grab the metapayload
        while let Some(result) = payloads.next().await {
            let payload = result?;
            match payload {
                Payload::Meta(m) => {
                    metadata.extend(m);
                    break;
                }
                _ => {}
            }
        }

        // Whack it into the cobbler
        let pkg = Meta::from_stone_payload(&metadata)?;
        let id = pkg.id();
        self.id_to_path.insert(id, path.display().to_string());
        self.packages.insert(pkg.id(), pkg);

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] stone::read::Error),

    #[error("metadata: {0}")]
    Metadata(#[from] MissingMetaError),

    #[error("unspecified error")]
    Unspecified,
}
