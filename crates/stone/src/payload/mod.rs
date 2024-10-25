// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

mod attribute;
mod content;
mod index;
pub mod layout;
pub mod meta;

use std::io::{self, Read, Write};

use thiserror::Error;

use crate::ext::{ReadExt, WriteExt};

pub use self::attribute::StonePayloadAttribute;
pub use self::content::StonePayloadContent;
pub use self::index::StonePayloadIndex;
pub use self::layout::{StonePayloadLayout, StonePayloadLayoutEntry, StonePayloadLayoutFileType};
pub use self::meta::{StonePayloadMeta, StonePayloadMetaDependency, StonePayloadMetaKind, StonePayloadMetaTag};

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
#[strum(serialize_all = "kebab-case")]
#[repr(u8)]
pub enum StonePayloadKind {
    // The Metadata store
    Meta = 1,
    // File store, i.e. hash indexed
    Content = 2,
    // Map Files to Disk with basic UNIX permissions + types
    Layout = 3,
    // For indexing the deduplicated store
    Index = 4,
    // Attribute storage
    Attributes = 5,
    // For Writer interim
    Dumb = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
#[strum(serialize_all = "kebab-case")]
#[repr(u8)]
pub enum StonePayloadCompression {
    // Payload has no compression
    None = 1,
    // Payload uses ZSTD compression
    Zstd = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct StonePayloadHeader {
    pub stored_size: u64,
    pub plain_size: u64,
    pub checksum: [u8; 8],
    pub num_records: usize,
    pub version: u16,
    pub kind: StonePayloadKind,
    pub compression: StonePayloadCompression,
}

impl StonePayloadHeader {
    pub fn decode<R: Read>(mut reader: R) -> Result<Self, StonePayloadDecodeError> {
        let stored_size = reader.read_u64()?;
        let plain_size = reader.read_u64()?;
        let checksum = reader.read_array()?;
        let num_records = reader.read_u32()? as usize;
        let version = reader.read_u16()?;

        let kind = match reader.read_u8()? {
            1 => StonePayloadKind::Meta,
            2 => StonePayloadKind::Content,
            3 => StonePayloadKind::Layout,
            4 => StonePayloadKind::Index,
            5 => StonePayloadKind::Attributes,
            6 => StonePayloadKind::Dumb,
            k => return Err(StonePayloadDecodeError::UnknownKind(k)),
        };

        let compression = match reader.read_u8()? {
            1 => StonePayloadCompression::None,
            2 => StonePayloadCompression::Zstd,
            d => return Err(StonePayloadDecodeError::UnknownCompression(d)),
        };

        Ok(Self {
            stored_size,
            plain_size,
            checksum,
            num_records,
            version,
            kind,
            compression,
        })
    }

    pub fn encode<W: Write>(&self, writer: &mut W) -> Result<(), StonePayloadEncodeError> {
        writer.write_u64(self.stored_size)?;
        writer.write_u64(self.plain_size)?;
        writer.write_array(self.checksum)?;
        writer.write_u32(self.num_records as u32)?;
        writer.write_u16(self.version)?;
        writer.write_u8(self.kind as u8)?;
        writer.write_u8(self.compression as u8)?;

        Ok(())
    }
}

pub(crate) trait Record: Sized {
    fn decode<R: Read>(reader: R) -> Result<Self, StonePayloadDecodeError>;
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), StonePayloadEncodeError>;
    fn size(&self) -> usize;
}

pub(crate) fn decode_records<T: Record, R: Read>(
    mut reader: R,
    num_records: usize,
) -> Result<Vec<T>, StonePayloadDecodeError> {
    let mut records = Vec::with_capacity(num_records);

    for _ in 0..num_records {
        records.push(T::decode(&mut reader)?);
    }

    Ok(records)
}

pub(crate) fn encode_records<T: Record, W: Write>(
    writer: &mut W,
    records: &[T],
) -> Result<(), StonePayloadEncodeError> {
    for record in records {
        record.encode(writer)?;
    }
    Ok(())
}

pub(crate) fn records_total_size<T: Record>(records: &[T]) -> usize {
    records.iter().map(T::size).sum()
}

#[derive(Debug, Clone)]
pub struct StonePayload<T> {
    pub header: StonePayloadHeader,
    pub body: T,
}

#[derive(Debug, Error)]
pub enum StonePayloadDecodeError {
    #[error("Unknown header type: {0}")]
    UnknownKind(u8),
    #[error("Unknown header compression: {0}")]
    UnknownCompression(u8),
    #[error("Unknown metadata type: {0}")]
    UnknownMetaKind(u8),
    #[error("Unknown metadata tag: {0}")]
    UnknownMetaTag(u16),
    #[error("Unknown file type: {0}")]
    UnknownFileType(u8),
    #[error("Unknown dependency type: {0}")]
    UnknownDependency(u8),
    #[error("io")]
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum StonePayloadEncodeError {
    #[error("io")]
    Io(#[from] io::Error),
}
