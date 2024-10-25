// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
#![allow(dead_code)]

use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use thiserror::Error;

use crate::{
    payload, StoneHeader, StoneHeaderDecodeError, StonePayload, StonePayloadAttribute, StonePayloadCompression,
    StonePayloadContent, StonePayloadDecodeError, StonePayloadHeader, StonePayloadIndex, StonePayloadKind,
    StonePayloadLayout, StonePayloadMeta,
};

use self::zstd::Zstd;

mod digest;
mod zstd;

pub fn read<R: Read + Seek>(mut reader: R) -> Result<StoneReader<R>, StoneReadError> {
    let header = StoneHeader::decode(&mut reader).map_err(StoneReadError::HeaderDecode)?;

    Ok(StoneReader {
        header,
        reader,
        hasher: digest::Hasher::new(),

        #[cfg(feature = "ffi")]
        next_payload: 0,
    })
}

pub fn read_bytes(bytes: &[u8]) -> Result<StoneReader<Cursor<&[u8]>>, StoneReadError> {
    read(Cursor::new(bytes))
}

pub struct StoneReader<R> {
    pub header: StoneHeader,
    reader: R,
    hasher: digest::Hasher,

    #[cfg(feature = "ffi")]
    next_payload: u16,
}

impl<R: Read + Seek> StoneReader<R> {
    pub fn payloads(
        &mut self,
    ) -> Result<impl Iterator<Item = Result<StoneDecodedPayload, StoneReadError>> + '_, StoneReadError> {
        if self.reader.stream_position()? != StoneHeader::SIZE as u64 {
            // Rewind to start of payloads
            self.reader.seek(SeekFrom::Start(StoneHeader::SIZE as u64))?;
        }

        #[cfg(feature = "ffi")]
        {
            self.next_payload = self.header.num_payloads();
        }

        Ok((0..self.header.num_payloads())
            .flat_map(|_| StoneDecodedPayload::decode(&mut self.reader, &mut self.hasher).transpose()))
    }

    pub fn unpack_content<W>(
        &mut self,
        content: &StonePayload<StonePayloadContent>,
        writer: &mut W,
    ) -> Result<(), StoneReadError>
    where
        W: Write,
    {
        self.reader.seek(SeekFrom::Start(content.body.offset))?;
        self.hasher.reset();

        let mut hashed = digest::Reader::new(&mut self.reader, &mut self.hasher);
        let mut framed = (&mut hashed).take(content.header.stored_size);

        io::copy(
            &mut PayloadReader::new(&mut framed, content.header.compression)?,
            writer,
        )?;

        // Validate checksum
        validate_checksum(&self.hasher, &content.header)?;

        Ok(())
    }
}

#[cfg(feature = "ffi")]
impl<R: Read + Seek> StoneReader<R> {
    pub fn next_payload(&mut self) -> Result<Option<StoneDecodedPayload>, StoneReadError> {
        if self.next_payload < self.header.num_payloads() {
            let payload = StoneDecodedPayload::decode(&mut self.reader, &mut self.hasher)?;

            self.next_payload += 1;

            Ok(payload)
        } else {
            Ok(None)
        }
    }
}

enum PayloadReader<R: Read> {
    Plain(R),
    Zstd(Zstd<R>),
}

impl<R: Read> PayloadReader<R> {
    fn new(reader: R, compression: StonePayloadCompression) -> Result<Self, StoneReadError> {
        Ok(match compression {
            StonePayloadCompression::None => PayloadReader::Plain(reader),
            StonePayloadCompression::Zstd => PayloadReader::Zstd(Zstd::new(reader)?),
        })
    }
}

impl<R: Read> Read for PayloadReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            PayloadReader::Plain(reader) => reader.read(buf),
            PayloadReader::Zstd(reader) => reader.read(buf),
        }
    }
}

#[derive(Debug)]
pub enum StoneDecodedPayload {
    Meta(StonePayload<Vec<StonePayloadMeta>>),
    Attributes(StonePayload<Vec<StonePayloadAttribute>>),
    Layout(StonePayload<Vec<StonePayloadLayout>>),
    Index(StonePayload<Vec<StonePayloadIndex>>),
    Content(StonePayload<StonePayloadContent>),
}

impl StoneDecodedPayload {
    pub fn header(&self) -> &StonePayloadHeader {
        match self {
            StoneDecodedPayload::Meta(payload) => &payload.header,
            StoneDecodedPayload::Attributes(payload) => &payload.header,
            StoneDecodedPayload::Layout(payload) => &payload.header,
            StoneDecodedPayload::Index(payload) => &payload.header,
            StoneDecodedPayload::Content(payload) => &payload.header,
        }
    }

    fn decode<R: Read + Seek>(mut reader: R, hasher: &mut digest::Hasher) -> Result<Option<Self>, StoneReadError> {
        match StonePayloadHeader::decode(&mut reader) {
            Ok(header) => {
                hasher.reset();
                let mut hashed = digest::Reader::new(&mut reader, hasher);
                let mut framed = (&mut hashed).take(header.stored_size);

                let payload = match header.kind {
                    StonePayloadKind::Meta => StoneDecodedPayload::Meta(StonePayload {
                        header,
                        body: payload::decode_records(
                            PayloadReader::new(&mut framed, header.compression)?,
                            header.num_records,
                        )?,
                    }),
                    StonePayloadKind::Layout => StoneDecodedPayload::Layout(StonePayload {
                        header,
                        body: payload::decode_records(
                            PayloadReader::new(&mut framed, header.compression)?,
                            header.num_records,
                        )?,
                    }),
                    StonePayloadKind::Index => StoneDecodedPayload::Index(StonePayload {
                        header,
                        body: payload::decode_records(
                            PayloadReader::new(&mut framed, header.compression)?,
                            header.num_records,
                        )?,
                    }),
                    StonePayloadKind::Attributes => StoneDecodedPayload::Attributes(StonePayload {
                        header,
                        body: payload::decode_records(
                            PayloadReader::new(&mut framed, header.compression)?,
                            header.num_records,
                        )?,
                    }),
                    StonePayloadKind::Content => {
                        let offset = reader.stream_position()?;

                        // Skip past, these are read by user later
                        reader.seek(SeekFrom::Current(header.stored_size as i64))?;

                        StoneDecodedPayload::Content(StonePayload {
                            header,
                            body: StonePayloadContent { offset },
                        })
                    }
                    StonePayloadKind::Dumb => unimplemented!("??"),
                };

                // Validate hash for non-content payloads
                if !matches!(header.kind, StonePayloadKind::Content) {
                    validate_checksum(hasher, &header)?;
                }

                Ok(Some(payload))
            }
            Err(StonePayloadDecodeError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(error) => Err(StoneReadError::PayloadDecode(error)),
        }
    }

    pub fn meta(&self) -> Option<&StonePayload<Vec<StonePayloadMeta>>> {
        if let Self::Meta(meta) = self {
            Some(meta)
        } else {
            None
        }
    }

    pub fn attributes(&self) -> Option<&StonePayload<Vec<StonePayloadAttribute>>> {
        if let Self::Attributes(attributes) = self {
            Some(attributes)
        } else {
            None
        }
    }

    pub fn layout(&self) -> Option<&StonePayload<Vec<StonePayloadLayout>>> {
        if let Self::Layout(layouts) = self {
            Some(layouts)
        } else {
            None
        }
    }

    pub fn index(&self) -> Option<&StonePayload<Vec<StonePayloadIndex>>> {
        if let Self::Index(indices) = self {
            Some(indices)
        } else {
            None
        }
    }

    pub fn content(&self) -> Option<&StonePayload<StonePayloadContent>> {
        if let Self::Content(content) = self {
            Some(content)
        } else {
            None
        }
    }
}

fn validate_checksum(hasher: &digest::Hasher, header: &StonePayloadHeader) -> Result<(), StoneReadError> {
    let got = hasher.digest();
    let expected = u64::from_be_bytes(header.checksum);

    if got != expected {
        Err(StoneReadError::PayloadChecksum { got, expected })
    } else {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum StoneReadError {
    #[error("Multiple content payloads not allowed")]
    MultipleContent,
    #[error("header decode")]
    HeaderDecode(#[from] StoneHeaderDecodeError),
    #[error("payload decode")]
    PayloadDecode(#[from] StonePayloadDecodeError),
    #[error("payload checksum mismatch: got {got:02x}, expected {expected:02x}")]
    PayloadChecksum { got: u64, expected: u64 },
    #[error("io")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod test {
    use xxhash_rust::xxh3::xxh3_128;

    use crate::{StoneHeaderVersion, StonePayloadLayoutEntry};

    use super::*;

    /// Header for bash completion stone archive
    const BASH_TEST_STONE: [u8; 32] = [
        0x0, 0x6d, 0x6f, 0x73, 0x0, 0x4, 0x0, 0x0, 0x1, 0x0, 0x0, 0x2, 0x0, 0x0, 0x3, 0x0, 0x0, 0x4, 0x0, 0x0, 0x5,
        0x0, 0x0, 0x6, 0x0, 0x0, 0x7, 0x1, 0x0, 0x0, 0x0, 0x1,
    ];

    #[test]
    fn read_header() {
        let stone = read_bytes(&BASH_TEST_STONE).expect("valid stone");
        assert_eq!(stone.header.version(), StoneHeaderVersion::V1);
    }

    #[test]
    fn read_bash_completion() {
        let mut stone =
            read_bytes(include_bytes!("../../../../test/bash-completion-2.11-1-1-x86_64.stone")).expect("valid stone");
        assert_eq!(stone.header.version(), StoneHeaderVersion::V1);

        let payloads = stone
            .payloads()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .expect("seek payloads");

        let mut unpacked_content = vec![];

        if let Some(content) = payloads.iter().find_map(StoneDecodedPayload::content) {
            stone
                .unpack_content(content, &mut unpacked_content)
                .expect("valid content");

            for index in payloads
                .iter()
                .filter_map(StoneDecodedPayload::index)
                .flat_map(|p| &p.body)
            {
                let content = &unpacked_content[index.start as usize..index.end as usize];
                let digest = xxh3_128(content);
                assert_eq!(digest, index.digest);

                payloads
                    .iter()
                    .filter_map(StoneDecodedPayload::layout)
                    .flat_map(|p| &p.body)
                    .find(|layout| {
                        if let StonePayloadLayoutEntry::Regular(digest, _) = &layout.entry {
                            return *digest == index.digest;
                        }
                        false
                    })
                    .expect("layout exists");
            }
        }
    }
}
