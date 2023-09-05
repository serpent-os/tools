// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

/// Simple corruption check in the header, will be expanded for v2
const INTEGRITY_CHECK: [u8; 21] = [
    0, 0, 1, 0, 0, 2, 0, 0, 3, 0, 0, 4, 0, 0, 5, 0, 0, 6, 0, 0, 7,
];

/// Well known file type for a v1 stone container
///
/// Some types are now legacy as we're going to use Ion to define them.
///
#[repr(u8)]
pub enum FileType {
    /// Sanity: Unknown container type
    Unknown,

    /// Binary package
    Binary,

    /// Delta package
    Delta,

    /// (Legacy) repository index
    Repository,

    /// (Legacy) build manifest
    BuildManifest,
}

/// Header for the v1 format version
#[derive(Debug, Clone, Copy)]
pub struct Header {
    _todo: [u8; 24],
}

impl Header {
    pub fn decode(bytes: [u8; 24]) -> Self {
        Self { _todo: bytes }
    }

    pub fn encode(self) -> [u8; 24] {
        todo!();
    }
}
