// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use bitflags::bitflags;
use itertools::Itertools;

use crate::{Dependency, Provider};

bitflags! {
    /// Flags indicating the status of a [`Package`]
    #[derive(Debug, Clone,Copy, PartialEq, Eq)]
    pub struct Flags: u8 {
        /// No filter flags
        const NONE = 0;
        /// Package is available for installation
        const AVAILABLE = 1 << 1;
        /// Package is already installed
        const INSTALLED = 1 << 2;
        /// Available as from-source build
        const SOURCE = 1 << 3;
    }
}

/// Unique ID of a [`Package`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Id(String);

impl From<String> for Id {
    fn from(id: String) -> Self {
        Self(id)
    }
}

/// The name of a [`Package`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);

impl From<String> for Name {
    fn from(name: String) -> Self {
        Self(name)
    }
}

/// Metadata of a [`Package`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub name: Name,
    pub summary: String,
    pub description: String,
    pub release_number: u64,
    pub version_id: String,
    pub homepage: String,
    pub licenses: Vec<String>,
}

/// A [`Registry`] package
///
/// [`Registry`]: super::Registry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub id: Id,
    pub metadata: Metadata,
    pub dependencies: Vec<Dependency>,
    pub providers: Vec<Provider>,
    pub flags: Flags,
}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Package {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.metadata
            .release_number
            .cmp(&other.metadata.release_number)
            .reverse()
    }
}

/// Iterate packages in sorted order
pub struct Sorted<I>(I);

impl<I> Sorted<I>
where
    I: IntoIterator<Item = Package>,
{
    pub fn new(iter: I) -> Self {
        Self(iter)
    }

    /// Sort the iterator
    pub fn into_iter(self) -> impl Iterator<Item = Package> {
        self.0.into_iter().sorted()
    }
}
