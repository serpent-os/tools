// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{self, Read};
use thiserror::Error;

use crate::header::{self, AgnosticHeader};
use crate::{Header, Stone};

const HEADER_BYTES: usize = std::mem::size_of::<AgnosticHeader>();

pub fn from_bytes(bytes: &[u8]) -> Result<Stone, ReadError> {
    from_reader(bytes)
}

/// Create a new reader for the given byte sequence
pub fn from_reader<R: Read>(mut reader: R) -> Result<Stone, ReadError> {
    let mut header_bytes = [0u8; HEADER_BYTES];
    reader.read_exact(&mut header_bytes)?;

    let agnostic = AgnosticHeader::from(header_bytes);
    let header = Header::decode(agnostic).map_err(ReadError::HeaderDecode)?;

    let mut payload = vec![];
    reader.read_to_end(&mut payload)?;

    Ok(Stone { header, payload })
}

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("Stone must be >{HEADER_BYTES} bytes long")]
    NotEnoughBytes,
    #[error("failed to decode header: {0}")]
    HeaderDecode(#[from] header::DecodeError),
    #[error(transparent)]
    Io(io::Error),
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::UnexpectedEof => ReadError::NotEnoughBytes,
            _ => ReadError::Io(error),
        }
    }
}
