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

pub fn read<R: Read + Seek>(
    mut reader: R,
) -> Result<(Header, Vec<Payload>, ContentReader<R>), ReadError> {
    let header = Header::decode(&mut reader).map_err(ReadError::HeaderDecode)?;

    let mut payloads = vec![];

    while let Some(payload) = Payload::decode(&mut reader)? {
        payloads.push(payload);
    }

    Ok((header, payloads, ContentReader(reader)))
}

pub fn read_bytes(
    bytes: &[u8],
) -> Result<(Header, Vec<Payload>, ContentReader<Cursor<&[u8]>>), ReadError> {
    read(Cursor::new(bytes))
}

pub struct ContentReader<R>(R);

enum PayloadReader<'a, R: Read> {
    Plain(&'a mut R),
    Zstd(Zstd<'a, io::Take<&'a mut R>>),
}

impl<'a, R: Read> PayloadReader<'a, R> {
    fn new(reader: &'a mut R, length: u64, compression: Compression) -> Result<Self, ReadError> {
        Ok(match compression {
            Compression::None => PayloadReader::Plain(reader),
            Compression::Zstd => PayloadReader::Zstd(Zstd::new(reader.take(length))?),
        })
    }
}

impl<'a, R: Read> Read for PayloadReader<'a, R> {
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
    fn decode<R: Read + Seek>(mut reader: R) -> Result<Option<Self>, ReadError> {
        match payload::Header::decode(&mut reader) {
            Ok(header) => {
                let payload = match header.kind {
                    payload::Kind::Meta => Payload::Meta(payload::decode_records(
                        PayloadReader::new(&mut reader, header.stored_size, header.compression)?,
                        header.num_records,
                    )?),
                    payload::Kind::Layout => Payload::Layout(payload::decode_records(
                        PayloadReader::new(&mut reader, header.stored_size, header.compression)?,
                        header.num_records,
                    )?),
                    payload::Kind::Index => Payload::Index(payload::decode_records(
                        PayloadReader::new(&mut reader, header.stored_size, header.compression)?,
                        header.num_records,
                    )?),
                    payload::Kind::Attributes => Payload::Attributes(payload::decode_records(
                        PayloadReader::new(&mut reader, header.stored_size, header.compression)?,
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
            Err(error) => Err(ReadError::PayloadDecode(error)),
        }
    }
}

#[derive(Debug)]
pub struct Content {
    offset: u64,
    length: u64,
    compression: Compression,
}

impl Content {
    pub fn load<R, W>(self, reader: &mut ContentReader<R>, writer: &mut W) -> Result<(), ReadError>
    where
        R: Read + Seek,
        W: Write,
    {
        reader.0.seek(SeekFrom::Start(self.offset))?;

        let mut reader = PayloadReader::new(&mut reader.0, self.length, self.compression)?;

        io::copy(&mut reader, writer)?;

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ReadError {
    #[error("failed to decode header: {0}")]
    HeaderDecode(#[from] header::DecodeError),
    #[error("failed to decode payload header: {0}")]
    PayloadDecode(#[from] payload::DecodeError),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod test {
    use super::*;

    /// Header for bash completion stone archive
    const BASH_TEST_STONE: [u8; 32] = [
        0x0, 0x6d, 0x6f, 0x73, 0x0, 0x4, 0x0, 0x0, 0x1, 0x0, 0x0, 0x2, 0x0, 0x0, 0x3, 0x0, 0x0,
        0x4, 0x0, 0x0, 0x5, 0x0, 0x0, 0x6, 0x0, 0x0, 0x7, 0x1, 0x0, 0x0, 0x0, 0x1,
    ];

    #[test]
    fn read_header() {
        let (header, _, _) = read_bytes(&BASH_TEST_STONE).expect("valid stone");
        assert_eq!(header.version(), header::Version::V1);
    }

    #[test]
    fn read_bash_completion() {
        let (header, payloads, _) = read_bytes(include_bytes!(
            "../test/bash-completion-2.11-1-1-x86_64.stone"
        ))
        .expect("valid stone");
        assert_eq!(header.version(), header::Version::V1);
        assert_eq!(payloads.len(), 4);
    }
}
