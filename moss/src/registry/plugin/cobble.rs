// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::fs::File;
use std::io::{self, Seek};
use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

use crate::package::{self, Meta, MissingMetaFieldError, Package};
use crate::Provider;
use sha2::{Digest, Sha256};
use stone::read::PayloadKind;
use thiserror::Error;

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Cobble {
    // Storage of local packages
    packages: HashMap<package::Id, State>,
}

impl Cobble {
    /// Add a package to the cobble set
    pub fn add_package(&mut self, path: impl Into<PathBuf>) -> Result<package::Id, Error> {
        let path = path.into();

        let mut file = File::open(&path)?;

        // Hash file to SHA256 and get size
        let (file_size, file_hash) = stat_file(&file)?;

        // Reset file reader to read it as a stone
        file.seek(io::SeekFrom::Start(0))?;

        // Read file as stone
        let mut reader = stone::read(&mut file)?;
        let mut payloads = reader.payloads()?;

        // Grab the metapayload
        let metadata = payloads
            .find_map(|result| {
                if let Ok(PayloadKind::Meta(meta)) = result {
                    Some(meta)
                } else {
                    None
                }
            })
            .ok_or(Error::MissingMetaPayload)?;

        // Update meta with uri, hash and size
        let mut meta = Meta::from_stone_payload(&metadata.body)?;
        meta.uri = Some(format!("file://{}", path.canonicalize()?.display()));
        meta.hash = Some(file_hash);
        meta.download_size = Some(file_size);

        // Create a package ID from the hashed path
        let id = path_hash(&path);

        // Whack it into the cobbler
        self.packages.insert(id.clone(), State { path, meta });

        Ok(id)
    }

    pub fn package(&self, id: &package::Id) -> Option<Package> {
        self.packages.get(id).map(|state| state.package(id.clone()))
    }

    fn query(&self, flags: package::Flags, filter: impl Fn(&Meta) -> bool) -> Vec<Package> {
        if flags.available {
            self.packages
                .iter()
                .filter(|(_, state)| filter(&state.meta))
                .map(|(id, state)| state.package(id.clone()))
                .collect()
        } else {
            vec![]
        }
    }

    pub fn list(&self, flags: package::Flags) -> Vec<Package> {
        self.query(flags, |_| true)
    }

    pub fn query_keyword(&self, keyword: &str, flags: package::Flags) -> Vec<Package> {
        self.query(flags, |meta| {
            meta.name.contains(keyword) || meta.summary.contains(keyword)
        })
    }

    pub fn query_provider(&self, provider: &Provider, flags: package::Flags) -> Vec<Package> {
        self.query(flags, |meta| meta.providers.contains(provider))
    }

    pub fn query_name(&self, package_name: &package::Name, flags: package::Flags) -> Vec<Package> {
        self.query(flags, |meta| meta.name == *package_name)
    }

    pub fn priority(&self) -> u64 {
        u64::MAX - 1
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
            flags: package::Flags::new().with_available(),
        }
    }
}

/// Hashes path into a SHA256 to use as a [`package::Id`]
fn path_hash(path: &Path) -> package::Id {
    let mut hasher = Sha256::new();

    hasher.update(path.as_os_str().as_encoded_bytes());

    hex::encode(hasher.finalize()).into()
}

fn stat_file(mut file: &File) -> Result<(u64, String), io::Error> {
    let mut hasher = Sha256::new();

    let len = io::copy(&mut file, &mut hasher)?;

    Ok((len, hex::encode(hasher.finalize())))
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Missing metadata payload")]
    MissingMetaPayload,

    #[error("stone read")]
    StoneRead(#[from] stone::read::Error),

    #[error("io")]
    Io(#[from] io::Error),

    #[error("metadata")]
    Metadata(#[from] MissingMetaFieldError),
}
