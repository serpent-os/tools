// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::Read;

use super::{DecodeError, Record};
use crate::ReadExt;

/// An IndexEntry (a series of sequential entries within the IndexPayload)
/// record offsets to unique files within the ContentPayload when decompressed
/// This is used to split the file into the content store on disk before promoting
/// to a transaction
#[derive(Debug, Clone, Copy)]
pub struct Index {
    /// Start pf the entry within the ContentPayload
    pub start: u64,

    /// End pointer, remove start for length
    pub end: u64,

    /// XXH3_128 hash
    pub digest: u128,
}

impl Record for Index {
    fn decode<R: Read>(mut reader: R) -> Result<Self, DecodeError> {
        let start = reader.read_u64()?;
        let end = reader.read_u64()?;
        let digest = reader.read_u128()?;

        Ok(Self { start, end, digest })
    }
}
