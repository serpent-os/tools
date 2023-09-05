// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use thiserror::Error;

pub mod v1;

/// Well defined magic field for a stone header
pub const STONE_MAGIC: u32 = 0x006d6f73;

/// Format versions are defined as u32, to allow further mangling in future
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V1 = 1,
}

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

impl From<[u8; 32]> for AgnosticHeader {
    fn from(bytes: [u8; 32]) -> Self {
        let (magic, rest) = bytes.split_at(4);
        let (data, version) = rest.split_at(24);

        AgnosticHeader {
            magic: magic.try_into().unwrap(),
            data: data.try_into().unwrap(),
            version: version.try_into().unwrap(),
        }
    }
}

pub enum Header {
    V1(v1::Header),
}

impl Header {
    pub fn version(&self) -> Version {
        match self {
            Header::V1(_) => Version::V1,
        }
    }

    pub fn encode(self) -> AgnosticHeader {
        let magic = u32::to_be_bytes(STONE_MAGIC);
        let version = u32::to_be_bytes(self.version() as u32);

        let data = match self {
            Header::V1(v1) => v1.encode(),
        };

        AgnosticHeader {
            magic,
            data,
            version,
        }
    }

    pub fn decode(header: AgnosticHeader) -> Result<Self, DecodeError> {
        if STONE_MAGIC != u32::from_be_bytes(header.magic) {
            return Err(DecodeError::InvalidMagic);
        }

        let version = match u32::from_be_bytes(header.version) {
            1 => Version::V1,
            v => return Err(DecodeError::UnknownVersion(v)),
        };

        Ok(match version {
            Version::V1 => Self::V1(v1::Header::decode(header.data)),
        })
    }
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("Invalid magic")]
    InvalidMagic,
    #[error("Unknown version: {0}")]
    UnknownVersion(u32),
}
