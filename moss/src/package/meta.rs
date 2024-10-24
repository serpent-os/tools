// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeSet;

use derive_more::{AsRef, Display, From, Into};
use stone::{StonePayloadMetaBody, StonePayloadMetaKind, StonePayloadMetaTag};
use thiserror::Error;

use crate::{dependency, Dependency, Provider};

/// A package identifier constructed from metadata fields
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Display)]
pub struct Id(pub(super) String);

/// The name of a [`super::Package`]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, AsRef, From, Into, Display)]
pub struct Name(String);

impl Name {
    pub fn contains(&self, text: &str) -> bool {
        self.0.contains(text)
    }
}

/// The metadata of a [`super::Package`]
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
    pub dependencies: BTreeSet<Dependency>,
    /// All providers, including name()
    pub providers: BTreeSet<Provider>,
    /// All providers that conflict with this package
    pub conflicts: BTreeSet<Provider>,
    /// If relevant: uri to fetch from
    pub uri: Option<String>,
    /// If relevant: hash for the download
    pub hash: Option<String>,
    /// How big is this package in the repo..?
    pub download_size: Option<u64>,
}

impl Meta {
    pub fn from_stone_payload(payload: &[StonePayloadMetaBody]) -> Result<Self, MissingMetaFieldError> {
        let name = find_meta_string(payload, StonePayloadMetaTag::Name)?;
        let version_identifier = find_meta_string(payload, StonePayloadMetaTag::Version)?;
        let source_release = find_meta_u64(payload, StonePayloadMetaTag::Release)?;
        let build_release = find_meta_u64(payload, StonePayloadMetaTag::BuildRelease)?;
        let architecture = find_meta_string(payload, StonePayloadMetaTag::Architecture)?;
        let summary = find_meta_string(payload, StonePayloadMetaTag::Summary)?;
        let description = find_meta_string(payload, StonePayloadMetaTag::Description)?;
        let source_id = find_meta_string(payload, StonePayloadMetaTag::SourceID)?;
        let homepage = find_meta_string(payload, StonePayloadMetaTag::Homepage)?;
        let uri = find_meta_string(payload, StonePayloadMetaTag::PackageURI).ok();
        let hash = find_meta_string(payload, StonePayloadMetaTag::PackageHash).ok();
        let download_size = find_meta_u64(payload, StonePayloadMetaTag::PackageSize).ok();

        let licenses = payload
            .iter()
            .filter_map(|meta| meta_string(meta, StonePayloadMetaTag::License))
            .collect();
        let dependencies = payload.iter().filter_map(meta_dependency).collect();
        let providers = payload
            .iter()
            .filter_map(meta_provider)
            // Add package name as provider
            .chain(Some(Provider {
                kind: dependency::Kind::PackageName,
                name: name.clone(),
            }))
            .collect();
        let conflicts = payload.iter().filter_map(meta_conflict).collect();

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
            conflicts,
            uri,
            hash,
            download_size,
        })
    }

    pub fn to_stone_payload(self) -> Vec<StonePayloadMetaBody> {
        vec![
            (
                StonePayloadMetaTag::Name,
                StonePayloadMetaKind::String(self.name.to_string()),
            ),
            (
                StonePayloadMetaTag::Version,
                StonePayloadMetaKind::String(self.version_identifier),
            ),
            (
                StonePayloadMetaTag::Release,
                StonePayloadMetaKind::Uint64(self.source_release),
            ),
            (
                StonePayloadMetaTag::BuildRelease,
                StonePayloadMetaKind::Uint64(self.build_release),
            ),
            (
                StonePayloadMetaTag::Architecture,
                StonePayloadMetaKind::String(self.architecture),
            ),
            (StonePayloadMetaTag::Summary, StonePayloadMetaKind::String(self.summary)),
            (
                StonePayloadMetaTag::Description,
                StonePayloadMetaKind::String(self.description),
            ),
            (
                StonePayloadMetaTag::SourceID,
                StonePayloadMetaKind::String(self.source_id),
            ),
            (
                StonePayloadMetaTag::Homepage,
                StonePayloadMetaKind::String(self.homepage),
            ),
        ]
        .into_iter()
        .chain(
            self.uri
                .map(|uri| (StonePayloadMetaTag::PackageURI, StonePayloadMetaKind::String(uri))),
        )
        .chain(
            self.hash
                .map(|hash| (StonePayloadMetaTag::PackageHash, StonePayloadMetaKind::String(hash))),
        )
        .chain(
            self.download_size
                .map(|size| (StonePayloadMetaTag::PackageSize, StonePayloadMetaKind::Uint64(size))),
        )
        .chain(
            self.licenses
                .into_iter()
                .map(|license| (StonePayloadMetaTag::License, StonePayloadMetaKind::String(license))),
        )
        .chain(self.dependencies.into_iter().map(|dep| {
            (
                StonePayloadMetaTag::Depends,
                StonePayloadMetaKind::Dependency(dep.kind.into(), dep.name),
            )
        }))
        .chain(
            self.providers
                .into_iter()
                // We re-add this on ingestion / it's implied
                .filter(|provider| provider.kind != dependency::Kind::PackageName)
                .map(|provider| {
                    (
                        StonePayloadMetaTag::Provides,
                        StonePayloadMetaKind::Provider(provider.kind.into(), provider.name),
                    )
                }),
        )
        .chain(
            self.conflicts
                .into_iter()
                // We re-add this on ingestion / it's implied
                .map(|conflict| {
                    (
                        StonePayloadMetaTag::Conflicts,
                        StonePayloadMetaKind::Provider(conflict.kind.into(), conflict.name),
                    )
                }),
        )
        .map(|(tag, kind)| StonePayloadMetaBody { tag, kind })
        .collect()
    }

    /// Return a reusable ID
    pub fn id(&self) -> Id {
        Id(format!(
            "{}-{}-{}.{}",
            &self.name.0, &self.version_identifier, &self.source_release, &self.architecture
        ))
    }
}

fn find_meta_string(meta: &[StonePayloadMetaBody], tag: StonePayloadMetaTag) -> Result<String, MissingMetaFieldError> {
    meta.iter()
        .find_map(|meta| meta_string(meta, tag))
        .ok_or(MissingMetaFieldError(tag))
}

fn find_meta_u64(meta: &[StonePayloadMetaBody], tag: StonePayloadMetaTag) -> Result<u64, MissingMetaFieldError> {
    meta.iter()
        .find_map(|meta| meta_u64(meta, tag))
        .ok_or(MissingMetaFieldError(tag))
}

fn meta_u64(meta: &StonePayloadMetaBody, tag: StonePayloadMetaTag) -> Option<u64> {
    if meta.tag == tag {
        Some(match meta.kind {
            StonePayloadMetaKind::Int8(i) => i as _,
            StonePayloadMetaKind::Uint8(i) => i as _,
            StonePayloadMetaKind::Int16(i) => i as _,
            StonePayloadMetaKind::Uint16(i) => i as _,
            StonePayloadMetaKind::Int32(i) => i as _,
            StonePayloadMetaKind::Uint32(i) => i as _,
            StonePayloadMetaKind::Int64(i) => i as _,
            StonePayloadMetaKind::Uint64(i) => i,
            _ => return None,
        })
    } else {
        None
    }
}

fn meta_string(meta: &StonePayloadMetaBody, tag: StonePayloadMetaTag) -> Option<String> {
    match (meta.tag, &meta.kind) {
        (meta_tag, StonePayloadMetaKind::String(value)) if meta_tag == tag => Some(value.clone()),
        _ => None,
    }
}

fn meta_dependency(meta: &StonePayloadMetaBody) -> Option<Dependency> {
    if let StonePayloadMetaKind::Dependency(kind, name) = meta.kind.clone() {
        Some(Dependency {
            kind: dependency::Kind::from(kind),
            name,
        })
    } else {
        None
    }
}

fn meta_provider(meta: &StonePayloadMetaBody) -> Option<Provider> {
    match (meta.tag, meta.kind.clone()) {
        (StonePayloadMetaTag::Provides, StonePayloadMetaKind::Provider(kind, name)) => Some(Provider {
            kind: dependency::Kind::from(kind),
            name: name.clone(),
        }),
        _ => None,
    }
}

fn meta_conflict(meta: &StonePayloadMetaBody) -> Option<Provider> {
    match (meta.tag, meta.kind.clone()) {
        (StonePayloadMetaTag::Conflicts, StonePayloadMetaKind::Provider(kind, name)) => Some(Provider {
            kind: dependency::Kind::from(kind),
            name: name.clone(),
        }),
        _ => None,
    }
}

#[derive(Debug, Error)]
#[error("Missing metadata field: {0:?}")]
pub struct MissingMetaFieldError(pub StonePayloadMetaTag);
