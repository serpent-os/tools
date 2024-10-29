// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{Read, Write};

use super::{Record, StonePayloadDecodeError, StonePayloadEncodeError};
use crate::ext::{ReadExt, WriteExt};

/// Layout entries record their target file type so they can be rebuilt on
/// the target installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
#[strum(serialize_all = "kebab-case")]
#[repr(u8)]
pub enum StonePayloadLayoutFileType {
    /// Regular file
    Regular = 1,

    /// Symbolic link (source + target set)
    Symlink,

    /// Directory node
    Directory,

    /// Character device
    CharacterDevice,

    /// Block device
    BlockDevice,

    /// FIFO node
    Fifo,

    /// UNIX Socket
    Socket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StonePayloadLayoutFile {
    Regular(u128, String),
    Symlink(String, String),
    Directory(String),

    // not properly supported
    CharacterDevice(String),
    BlockDevice(String),
    Fifo(String),
    Socket(String),
}

impl StonePayloadLayoutFile {
    fn source(&self) -> Vec<u8> {
        match self {
            StonePayloadLayoutFile::Regular(hash, _) => hash.to_be_bytes().to_vec(),
            StonePayloadLayoutFile::Symlink(source, _) => source.as_bytes().to_vec(),
            StonePayloadLayoutFile::Directory(_) => vec![],
            StonePayloadLayoutFile::CharacterDevice(_) => vec![],
            StonePayloadLayoutFile::BlockDevice(_) => vec![],
            StonePayloadLayoutFile::Fifo(_) => vec![],
            StonePayloadLayoutFile::Socket(_) => vec![],
        }
    }

    pub fn target(&self) -> &str {
        match self {
            StonePayloadLayoutFile::Regular(_, target) => target,
            StonePayloadLayoutFile::Symlink(_, target) => target,
            StonePayloadLayoutFile::Directory(target) => target,
            StonePayloadLayoutFile::CharacterDevice(target) => target,
            StonePayloadLayoutFile::BlockDevice(target) => target,
            StonePayloadLayoutFile::Fifo(target) => target,
            StonePayloadLayoutFile::Socket(target) => target,
        }
    }

    pub fn file_type(&self) -> StonePayloadLayoutFileType {
        match self {
            StonePayloadLayoutFile::Regular(..) => StonePayloadLayoutFileType::Regular,
            StonePayloadLayoutFile::Symlink(..) => StonePayloadLayoutFileType::Symlink,
            StonePayloadLayoutFile::Directory(_) => StonePayloadLayoutFileType::Directory,
            StonePayloadLayoutFile::CharacterDevice(_) => StonePayloadLayoutFileType::CharacterDevice,
            StonePayloadLayoutFile::BlockDevice(_) => StonePayloadLayoutFileType::BlockDevice,
            StonePayloadLayoutFile::Fifo(_) => StonePayloadLayoutFileType::Fifo,
            StonePayloadLayoutFile::Socket(_) => StonePayloadLayoutFileType::Socket,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StonePayloadLayoutRecord {
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub tag: u32,
    pub file: StonePayloadLayoutFile,
}

impl Record for StonePayloadLayoutRecord {
    fn decode<R: Read>(mut reader: R) -> Result<Self, StonePayloadDecodeError> {
        let uid = reader.read_u32()?;
        let gid = reader.read_u32()?;
        let mode = reader.read_u32()?;
        let tag = reader.read_u32()?;

        let source_length = reader.read_u16()?;
        let target_length = reader.read_u16()?;
        let sanitize = |s: String| s.trim_end_matches('\0').to_owned();

        let file_type = match reader.read_u8()? {
            1 => StonePayloadLayoutFileType::Regular,
            2 => StonePayloadLayoutFileType::Symlink,
            3 => StonePayloadLayoutFileType::Directory,
            4 => StonePayloadLayoutFileType::CharacterDevice,
            5 => StonePayloadLayoutFileType::BlockDevice,
            6 => StonePayloadLayoutFileType::Fifo,
            7 => StonePayloadLayoutFileType::Socket,
            t => return Err(StonePayloadDecodeError::UnknownFileType(t)),
        };

        let _padding = reader.read_array::<11>()?;

        // Make the layout entry *usable*
        let entry = match file_type {
            // BUG: boulder stores xxh128 as le bytes not be
            StonePayloadLayoutFileType::Regular => {
                let source = reader.read_vec(source_length as usize)?;
                let hash = u128::from_be_bytes(source.try_into().unwrap());
                StonePayloadLayoutFile::Regular(hash, sanitize(reader.read_string(target_length as u64)?))
            }
            StonePayloadLayoutFileType::Symlink => StonePayloadLayoutFile::Symlink(
                sanitize(reader.read_string(source_length as u64)?),
                sanitize(reader.read_string(target_length as u64)?),
            ),
            StonePayloadLayoutFileType::Directory => {
                StonePayloadLayoutFile::Directory(sanitize(reader.read_string(target_length as u64)?))
            }
            _ => {
                if source_length > 0 {
                    let _ = reader.read_vec(source_length as usize);
                }
                unreachable!()
            }
        };

        Ok(Self {
            uid,
            gid,
            mode,
            tag,
            file: entry,
        })
    }

    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), StonePayloadEncodeError> {
        writer.write_u32(self.uid)?;
        writer.write_u32(self.gid)?;
        writer.write_u32(self.mode)?;
        writer.write_u32(self.tag)?;

        let source = self.file.source();
        let target = self.file.target();

        writer.write_u16(source.len() as u16)?;
        writer.write_u16(target.len() as u16)?;
        writer.write_u8(self.file.file_type() as u8)?;
        writer.write_array([0; 11])?;
        writer.write_all(&source)?;
        writer.write_all(target.as_bytes())?;

        Ok(())
    }

    fn size(&self) -> usize {
        4 + 4 + 4 + 4 + 2 + 2 + 1 + 11 + self.file.source().len() + self.file.target().len()
    }
}
