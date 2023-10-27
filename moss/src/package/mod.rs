// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use bitflags::bitflags;
use itertools::Itertools;

pub use self::meta::{Meta, MissingMetaError, Name};

pub mod meta;
pub mod render;

/// Unique ID of a [`Package`]
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(String);

impl From<String> for Id {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<Id> for String {
    fn from(id: Id) -> Self {
        id.0
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<Id> for meta::Id {
    fn from(id: Id) -> Self {
        meta::Id(id.0)
    }
}

impl From<meta::Id> for Id {
    fn from(id: meta::Id) -> Self {
        Self(id.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub id: Id,
    pub meta: Meta,
    pub flags: Flags,
}

impl Package {
    pub fn is_installed(&self) -> bool {
        self.flags.contains(Flags::INSTALLED)
    }
}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Package {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.meta
            .source_release
            .cmp(&other.meta.source_release)
            .reverse()
            .then_with(|| {
                self.meta
                    .build_release
                    .cmp(&other.meta.build_release)
                    .reverse()
            })
    }
}

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

/// Iterate packages in sorted order
pub struct Sorted<I>(I);

impl<I> Sorted<I> {
    pub fn new(iter: I) -> Self {
        Self(iter)
    }
}

/// Iterate in sorted order
impl<I> IntoIterator for Sorted<I>
where
    I: IntoIterator<Item = Package>,
{
    type Item = Package;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().sorted()
    }
}
