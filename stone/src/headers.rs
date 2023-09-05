// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

/// Well defined magic field for a stone header
pub const STONE_MAGIC: u32 = 0x006d6f73;

/// Format versions are defined as u32, to allow further mangling in future
#[repr(u32)]
pub enum Version {
    V1 = 1,
}

///
/// The stone format uses an agnostic approach requiring a valid magic field
/// in the first 4 bytes, and a version specifier in the last 4 bytes, using
/// big endian order.
///
/// When the version is decoded, we can create the appropriate, version-specific
/// reader implementation, ensuring the container format is extensible well into
/// the future
#[repr(C)]
pub struct AgnosticHeader {
    /// 4-bytes, BE (u32): Magic to quickly identify a stone file
    magic: [u8; 4],

    /// 24 bytes, version specific
    data: [u8; 24],

    /// 4-bytes, BE (u32): Format version used in the container
    version: [u8; 4],
}
