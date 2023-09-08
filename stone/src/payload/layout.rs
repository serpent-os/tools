// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::Read;

use super::{DecodeError, Record};
use crate::ReadExt;

/// Layout entries record their target file type so they can be rebuilt on
/// the target installation.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
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

// TODO: Strong types these fields
#[derive(Debug)]
pub struct Layout {
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub tag: u32,
    pub source: Option<Vec<u8>>,
    pub target: Vec<u8>,
    pub file_type: FileType,
}

impl Record for Layout {
    fn decode<R: Read>(mut reader: R) -> Result<Self, DecodeError> {
        let uid = reader.read_u32()?;
        let gid = reader.read_u32()?;
        let mode = reader.read_u32()?;
        let tag = reader.read_u32()?;

        let source_length = reader.read_u16()?;
        let target_length = reader.read_u16()?;

        let file_type = match reader.read_u8()? {
            1 => FileType::Regular,
            2 => FileType::Symlink,
            3 => FileType::Directory,
            4 => FileType::CharacterDevice,
            5 => FileType::BlockDevice,
            6 => FileType::Fifo,
            7 => FileType::Socket,
            t => return Err(DecodeError::UnknownFileType(t)),
        };

        let _padding = reader.read_array::<11>()?;

        let source = (source_length > 0)
            .then(|| reader.read_vec(source_length as usize))
            .transpose()?;

        let target = reader.read_vec(target_length as usize)?;

        Ok(Self {
            uid,
            gid,
            mode,
            tag,
            source,
            target,
            file_type,
        })
    }
}
