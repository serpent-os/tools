// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

const INTEGRITY_CHECK: [u8; 21] = [
    0, 0, 1, 0, 0, 2, 0, 0, 3, 0, 0, 4, 0, 0, 5, 0, 0, 6, 0, 0, 7,
];

///
/// Well known file type for a v1 stone container
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
