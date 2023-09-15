// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashSet;

use stone::payload;
use thiserror::Error;

use crate::{dependency, Dependency, Provider};

/// The name of a [`Package`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);

impl From<String> for Name {
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl From<Name> for String {
    fn from(name: Name) -> Self {
        name.0
    }
}

/// The metadata of a [`Package`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// Package name
    pub name: Name,
    /// Human readable version identifier
    pub version_identifier: String,
    /// Package release as set in stone.yml
    pub source_release: u64,
    /// Build machinery specific build release
    pub build_release: u64,
    /// Architecture this was built for
    pub architecture: String,
    /// Brief one line summary of the package
    pub summary: String,
    /// Description of the package
    pub description: String,
    /// The source-grouping ID
    pub source_id: String,
    /// Where'd we find this guy..
    pub homepage: String,
    /// Licenses this is available under
    pub licenses: Vec<String>,
    /// All dependencies
    pub dependencies: HashSet<Dependency>,
    /// All providers, including name()
    pub providers: HashSet<Provider>,
    /// If relevant: uri to fetch from
    pub uri: Option<String>,
    /// If relevant: hash for the download
    pub hash: Option<String>,
    /// How big is this package in the repo..?
    pub download_size: Option<u64>,
}

impl Meta {
    pub fn from_stone_payload(payload: &[stone::payload::Meta]) -> Result<Self, MissingMetaError> {
        let name = find_meta_string(payload, payload::meta::Tag::Name)?;
        let version_identifier = find_meta_string(payload, payload::meta::Tag::Version)?;
        let source_release = find_meta_u64(payload, payload::meta::Tag::Release)?;
        let build_release = find_meta_u64(payload, payload::meta::Tag::BuildRelease)?;
        let architecture = find_meta_string(payload, payload::meta::Tag::Architecture)?;
        let summary = find_meta_string(payload, payload::meta::Tag::Summary)?;
        let description = find_meta_string(payload, payload::meta::Tag::Description)?;
        let source_id = find_meta_string(payload, payload::meta::Tag::SourceID)?;
        let homepage = find_meta_string(payload, payload::meta::Tag::Homepage)?;
        let uri = find_meta_string(payload, payload::meta::Tag::PackageURI).ok();
        let hash = find_meta_string(payload, payload::meta::Tag::PackageHash).ok();
        let download_size = find_meta_u64(payload, payload::meta::Tag::PackageSize).ok();

        let licenses = payload
            .iter()
            .filter_map(|meta| meta_string(meta, payload::meta::Tag::License))
            .collect();
        let dependencies = payload
            .iter()
            .filter_map(|meta| meta_dependency(meta))
            .collect();
        let providers = payload
            .iter()
            .filter_map(|meta| meta_provider(meta))
            // Add package name as provider
            .chain(Some(Provider {
                kind: dependency::Kind::PackageName,
                name: name.clone(),
            }))
            .collect();

        Ok(Meta {
            name: Name::from(name),
            version_identifier,
            source_release,
            build_release,
            architecture,
            summary,
            description,
            source_id,
            homepage,
            licenses,
            dependencies,
            providers,
            uri,
            hash,
            download_size,
        })
    }
}

fn find_meta_string(
    meta: &[payload::Meta],
    tag: payload::meta::Tag,
) -> Result<String, MissingMetaError> {
    meta.iter()
        .find_map(|meta| meta_string(meta, tag))
        .ok_or(MissingMetaError(tag))
}

fn find_meta_u64(meta: &[payload::Meta], tag: payload::meta::Tag) -> Result<u64, MissingMetaError> {
    meta.iter()
        .find_map(|meta| meta_u64(meta, tag))
        .ok_or(MissingMetaError(tag))
}

fn meta_u64(meta: &payload::Meta, tag: payload::meta::Tag) -> Option<u64> {
    if meta.tag == tag {
        Some(match meta.kind {
            payload::meta::Kind::Int8(i) => i as _,
            payload::meta::Kind::Uint8(i) => i as _,
            payload::meta::Kind::Int16(i) => i as _,
            payload::meta::Kind::Uint16(i) => i as _,
            payload::meta::Kind::Int32(i) => i as _,
            payload::meta::Kind::Uint32(i) => i as _,
            payload::meta::Kind::Int64(i) => i as _,
            payload::meta::Kind::Uint64(i) => i,
            _ => return None,
        })
    } else {
        None
    }
}

fn meta_string(meta: &payload::Meta, tag: payload::meta::Tag) -> Option<String> {
    match (meta.tag, &meta.kind) {
        (meta_tag, payload::meta::Kind::String(value)) if meta_tag == tag => Some(value.clone()),
        _ => None,
    }
}

fn meta_dependency(meta: &payload::Meta) -> Option<Dependency> {
    if let payload::meta::Kind::Dependency(kind, name) = meta.kind.clone() {
        Some(Dependency {
            kind: dependency::Kind::from(kind),
            name,
        })
    } else {
        None
    }
}

fn meta_provider(meta: &payload::Meta) -> Option<Provider> {
    if let payload::meta::Kind::Provider(kind, name) = meta.kind.clone() {
        Some(Provider {
            kind: dependency::Kind::from(kind),
            name,
        })
    } else {
        None
    }
}

#[derive(Debug, Error)]
#[error("Missing metadata field: {0:?}")]
pub struct MissingMetaError(pub payload::meta::Tag);
