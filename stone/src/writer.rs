// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{self, Write};
use thiserror::Error;

use crate::header::AgnosticHeader;
use crate::Stone;

pub fn to_bytes(stone: &Stone) -> Result<Vec<u8>, WriteError> {
    let size = std::mem::size_of::<AgnosticHeader>() + stone.payload.len();

    let mut bytes = Vec::with_capacity(size);
    to_writer(stone, &mut bytes)?;

    Ok(bytes)
}

pub fn to_writer<W: Write>(stone: &Stone, writer: &mut W) -> Result<(), WriteError> {
    let header = stone.header.encode();

    writer.write_all(&header.magic)?;
    writer.write_all(&header.data)?;
    writer.write_all(&header.version)?;

    writer.write_all(&stone.payload)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error(transparent)]
    Io(#[from] io::Error),
}
