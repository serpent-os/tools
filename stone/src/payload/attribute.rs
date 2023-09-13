// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::Read;

use super::{DecodeError, Record};
use crate::ReadExt;

#[derive(Debug, Clone)]
pub struct Attribute {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

impl Record for Attribute {
    fn decode<R: Read>(mut reader: R) -> Result<Self, DecodeError> {
        let key_length = reader.read_u64()?;
        let value_length = reader.read_u64()?;

        let key = reader.read_vec(key_length as usize)?;
        let value = reader.read_vec(value_length as usize)?;

        Ok(Self { key, value })
    }
}
