// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::Read;

use super::{DecodeError, Record};
use crate::ReadExt;

#[derive(Debug, Clone, Copy)]
pub struct Index {
    pub start: u64,
    pub end: u64,
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
