// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use thiserror::Error;

use crate::header;
use crate::payload::{Attribute, Compression, Index, Layout, Meta};
use crate::{payload, Header};

use self::zstd::Zstd;

mod zstd;

pub fn read<R: Read + Seek>(mut reader: R) -> Result<Stone<R>, Error> {
    let header = Header::decode(&mut reader).map_err(Error::HeaderDecode)?;

    Ok(Stone { header, reader })
}

pub fn read_bytes(bytes: &[u8]) -> Result<Stone<Cursor<&[u8]>>, Error> {
    read(Cursor::new(bytes))
}

pub struct Stone<R> {
    pub header: Header,
    reader: R,
}

impl<R: Read + Seek> Stone<R> {
    pub fn payloads(&mut self) -> Result<impl Iterator<Item = Result<Payload, Error>> + '_, Error> {
        if self.reader.stream_position()? != Header::SIZE as u64 {
            // Rewind to start of payloads
            self.reader.seek(SeekFrom::Start(Header::SIZE as u64))?;
        }

        Ok((0..self.header.num_payloads())
            .flat_map(|_| Payload::decode(&mut self.reader).transpose()))
    }

    pub fn unpack_content<W: Write>(
        &mut self,
        content: &Content,
        writer: &mut W,
    ) -> Result<(), Error>
    where
        W: Write,
    {
        self.reader.seek(SeekFrom::Start(content.offset))?;

        let mut framed = (&mut self.reader).take(content.length);
        let mut reader = PayloadReader::new(&mut framed, content.compression)?;

        io::copy(&mut reader, writer)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Content {
    pub plain_size: u64,
    offset: u64,
    length: u64,
    compression: Compression,
}

enum PayloadReader<R: Read> {
    Plain(R),
    Zstd(Zstd<R>),
}

impl<R: Read> PayloadReader<R> {
    fn new(reader: R, compression: Compression) -> Result<Self, Error> {
        Ok(match compression {
            Compression::None => PayloadReader::Plain(reader),
            Compression::Zstd => PayloadReader::Zstd(Zstd::new(reader)?),
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
pub enum Payload {
    Meta(Vec<Meta>),
    Attributes(Vec<Attribute>),
    Layout(Vec<Layout>),
    Index(Vec<Index>),
    Content(Content),
}

impl Payload {
    fn decode<R: Read + Seek>(mut reader: R) -> Result<Option<Self>, Error> {
        match payload::Header::decode(&mut reader) {
            Ok(header) => {
                let mut framed = (&mut reader).take(header.stored_size);

                let payload = match header.kind {
                    payload::Kind::Meta => Payload::Meta(payload::decode_records(
                        PayloadReader::new(&mut framed, header.compression)?,
                        header.num_records,
                    )?),
                    payload::Kind::Layout => Payload::Layout(payload::decode_records(
                        PayloadReader::new(&mut framed, header.compression)?,
                        header.num_records,
                    )?),
                    payload::Kind::Index => Payload::Index(payload::decode_records(
                        PayloadReader::new(&mut framed, header.compression)?,
                        header.num_records,
                    )?),
                    payload::Kind::Attributes => Payload::Attributes(payload::decode_records(
                        PayloadReader::new(&mut framed, header.compression)?,
                        header.num_records,
                    )?),
                    payload::Kind::Content => {
                        let offset = reader.stream_position()?;
                        let length = header.stored_size;

                        // Skip past, these are read by user later
                        reader.seek(SeekFrom::Current(length as i64))?;

                        Payload::Content(Content {
                            plain_size: header.plain_size,
                            offset,
                            length,
                            compression: header.compression,
                        })
                    }
                    payload::Kind::Dumb => unimplemented!("??"),
                };

                Ok(Some(payload))
            }
            Err(payload::DecodeError::Io(error))
                if error.kind() == io::ErrorKind::UnexpectedEof =>
            {
                Ok(None)
            }
            Err(error) => Err(Error::PayloadDecode(error)),
        }
    }

    pub fn meta(&self) -> Option<&[Meta]> {
        if let Self::Meta(meta) = self {
            Some(meta)
        } else {
            None
        }
    }

    pub fn attributes(&self) -> Option<&[Attribute]> {
        if let Self::Attributes(attributes) = self {
            Some(attributes)
        } else {
            None
        }
    }

    pub fn layout(&self) -> Option<&[Layout]> {
        if let Self::Layout(layouts) = self {
            Some(layouts)
        } else {
            None
        }
    }

    pub fn index(&self) -> Option<&[Index]> {
        if let Self::Index(indicies) = self {
            Some(indicies)
        } else {
            None
        }
    }

    pub fn content(&self) -> Option<&Content> {
        if let Self::Content(content) = self {
            Some(content)
        } else {
            None
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Multiple content payloads not allowed")]
    MultipleContent,
    #[error("failed to decode header: {0}")]
    HeaderDecode(#[from] header::DecodeError),
    #[error("failed to decode payload: {0}")]
    PayloadDecode(#[from] payload::DecodeError),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod test {
    use xxhash_rust::xxh3::xxh3_128;

    use crate::payload::layout::Entry;

    use super::*;

    /// Header for bash completion stone archive
    const BASH_TEST_STONE: [u8; 32] = [
        0x0, 0x6d, 0x6f, 0x73, 0x0, 0x4, 0x0, 0x0, 0x1, 0x0, 0x0, 0x2, 0x0, 0x0, 0x3, 0x0, 0x0,
        0x4, 0x0, 0x0, 0x5, 0x0, 0x0, 0x6, 0x0, 0x0, 0x7, 0x1, 0x0, 0x0, 0x0, 0x1,
    ];

    #[test]
    fn read_header() {
        let stone = read_bytes(&BASH_TEST_STONE).expect("valid stone");
        assert_eq!(stone.header.version(), header::Version::V1);
    }

    #[test]
    fn read_bash_completion() {
        let mut stone = read_bytes(include_bytes!(
            "../../test/bash-completion-2.11-1-1-x86_64.stone"
        ))
        .expect("valid stone");
        assert_eq!(stone.header.version(), header::Version::V1);

        let payloads = stone
            .payloads()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .expect("seek payloads");

        let mut unpacked_content = vec![];

        if let Some(content) = payloads.iter().find_map(Payload::content) {
            stone
                .unpack_content(content, &mut unpacked_content)
                .expect("valid content");

            for index in payloads.iter().filter_map(Payload::index).flatten() {
                let content = &unpacked_content[index.start as usize..index.end as usize];
                let digest = xxh3_128(content);
                assert_eq!(digest, index.digest);

                payloads
                    .iter()
                    .filter_map(Payload::layout)
                    .flatten()
                    .find(|layout| {
                        if let Entry::Regular(digest, _) = &layout.entry {
                            return *digest == index.digest;
                        }
                        false
                    })
                    .expect("layout exists");
            }
        }
    }
}
