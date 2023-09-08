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

    let mut metadata = vec![];
    let mut attributes = vec![];
    let mut layouts = vec![];
    let mut indices = vec![];
    let mut content = None;

    while let Some(payload) = Payload::decode(&mut reader)? {
        match payload {
            Payload::Meta(m) => metadata.extend(m),
            Payload::Attributes(a) => attributes.extend(a),
            Payload::Layout(l) => layouts.extend(l),
            Payload::Index(i) => indices.extend(i),
            Payload::Content(c) => {
                if content.is_some() {
                    return Err(Error::MultipleContent);
                }
                content = Some(c);
            }
        }
    }

    Ok(Stone {
        reader,
        header,
        metadata,
        attributes,
        layouts,
        indices,
        content,
    })
}

pub fn read_bytes(bytes: &[u8]) -> Result<Stone<Cursor<&[u8]>>, Error> {
    read(Cursor::new(bytes))
}

pub struct Stone<R> {
    reader: R,
    pub header: Header,
    pub metadata: Vec<Meta>,
    pub attributes: Vec<Attribute>,
    pub layouts: Vec<Layout>,
    pub indices: Vec<Index>,
    pub content: Option<Content>,
}

impl<R: Read + Seek> Stone<R> {
    pub fn unpack_content<W: Write>(
        &mut self,
        content: Content,
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
enum Payload {
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

    use crate::payload::layout::FileType;

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
            "../test/bash-completion-2.11-1-1-x86_64.stone"
        ))
        .expect("valid stone");
        assert_eq!(stone.header.version(), header::Version::V1);

        let mut content = vec![];
        stone
            .unpack_content(stone.content.unwrap(), &mut content)
            .expect("valid content");

        for index in stone.indices {
            let content = &content[index.start as usize..index.end as usize];
            let digest = xxh3_128(content);
            assert_eq!(digest, index.digest);

            let layout = stone
                .layouts
                .iter()
                .find(|layout| layout.source.as_deref() == Some(&index.digest.to_be_bytes()))
                .expect("layout exists");
            assert_eq!(layout.file_type, FileType::Regular);
        }
    }
}
