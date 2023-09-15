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

#[derive(Debug, Clone)]
pub enum LayoutEntry {
    Regular(u128, String),
    Symlink(String, String),
    Directory(String),

    // not properly supported
    CharacterDevice(String),
    BlockDevice(String),
    Fifo(String),
    Socket(String),
}

// TODO: Strong types these fields
#[derive(Debug, Clone)]
pub struct Layout {
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub tag: u32,
    pub entry: LayoutEntry,
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

        // Make the layout entry *usable*
        let entry = match file_type {
            // BUG: boulder stores xxh128 as le bytes not be
            FileType::Regular => {
                let source = reader.read_vec(source_length as usize)?;
                let hash = u128::from_be_bytes(source.try_into().unwrap());
                LayoutEntry::Regular(hash, reader.read_string(target_length as u64)?)
            }
            FileType::Symlink => LayoutEntry::Symlink(
                reader.read_string(source_length as u64)?,
                reader.read_string(target_length as u64)?,
            ),
            FileType::Directory => {
                LayoutEntry::Directory(reader.read_string(target_length as u64)?)
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
            entry,
        })
    }
}
