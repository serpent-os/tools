// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io::{Read, Write};

use super::{Record, StonePayloadDecodeError, StonePayloadEncodeError};
use crate::ext::{ReadExt, WriteExt};

/// The Meta payload contains a series of sequential records with
/// strong types and context tags, i.e. their use such as Name.
/// These record all metadata for every .stone packages and provide
/// no content
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StonePayloadMeta {
    pub tag: StonePayloadMetaTag,
    pub kind: StonePayloadMetaKind,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum StonePayloadMetaDependency {
    /// Just the plain name of a package
    #[strum(serialize = "name")]
    PackageName = 0,

    /// A soname based dependency
    #[strum(serialize = "soname")]
    SharedLibrary,

    /// A pkgconfig `.pc` based dependency
    PkgConfig,

    /// Special interpreter (PT_INTERP/etc) to run the binaries
    Interpreter,

    /// A CMake module
    CMake,

    /// A Python module
    Python,

    /// A binary in /usr/bin
    Binary,

    /// A binary in /usr/sbin
    #[strum(serialize = "sysbinary")]
    SystemBinary,

    /// An emul32-compatible pkgconfig .pc dependency (lib32/*.pc)
    PkgConfig32,
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StonePayloadMetaKind {
    Int8(i8),
    Uint8(u8),
    Int16(i16),
    Uint16(u16),
    Int32(i32),
    Uint32(u32),
    Int64(i64),
    Uint64(u64),
    String(String),
    Dependency(StonePayloadMetaDependency, String),
    Provider(StonePayloadMetaDependency, String),
}

impl StonePayloadMetaKind {
    fn size(&self) -> usize {
        match self {
            StonePayloadMetaKind::Int8(_) => size_of::<i8>(),
            StonePayloadMetaKind::Uint8(_) => size_of::<u8>(),
            StonePayloadMetaKind::Int16(_) => size_of::<i16>(),
            StonePayloadMetaKind::Uint16(_) => size_of::<u16>(),
            StonePayloadMetaKind::Int32(_) => size_of::<i32>(),
            StonePayloadMetaKind::Uint32(_) => size_of::<u32>(),
            StonePayloadMetaKind::Int64(_) => size_of::<i64>(),
            StonePayloadMetaKind::Uint64(_) => size_of::<u64>(),
            // nul terminator
            StonePayloadMetaKind::String(s) => s.len() + 1,
            // Plus dep size & nul terminator
            StonePayloadMetaKind::Dependency(_, s) => s.len() + 2,
            // Plus dep size & nul terminator
            StonePayloadMetaKind::Provider(_, s) => s.len() + 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
#[strum(serialize_all = "kebab-case")]
#[repr(u16)]
pub enum StonePayloadMetaTag {
    // Name of the package
    Name = 1,
    // Architecture of the package
    Architecture = 2,
    // Version of the package
    Version = 3,
    // Summary of the package
    Summary = 4,
    // Description of the package
    Description = 5,
    // Homepage for the package
    Homepage = 6,
    // ID for the source package, used for grouping
    SourceID = 7,
    // Runtime dependencies
    Depends = 8,
    // Provides some capability or name
    Provides = 9,
    // Conflicts with some capability or name
    Conflicts = 10,
    // Release number for the package
    Release = 11,
    // SPDX license identifier
    License = 12,
    // Currently recorded build number
    BuildRelease = 13,
    // Repository index specific (relative URI)
    PackageURI = 14,
    // Repository index specific (Package hash)
    PackageHash = 15,
    // Repository index specific (size on disk)
    PackageSize = 16,
    // A Build Dependency
    BuildDepends = 17,
    // Upstream URI for the source
    SourceURI = 18,
    // Relative path for the source within the upstream URI
    SourcePath = 19,
    // Ref/commit of the upstream source
    SourceRef = 20,
}

/// Helper to decode a dependency's encoded kind
fn decode_dependency(i: u8) -> Result<StonePayloadMetaDependency, StonePayloadDecodeError> {
    let result = match i {
        0 => StonePayloadMetaDependency::PackageName,
        1 => StonePayloadMetaDependency::SharedLibrary,
        2 => StonePayloadMetaDependency::PkgConfig,
        3 => StonePayloadMetaDependency::Interpreter,
        4 => StonePayloadMetaDependency::CMake,
        5 => StonePayloadMetaDependency::Python,
        6 => StonePayloadMetaDependency::Binary,
        7 => StonePayloadMetaDependency::SystemBinary,
        8 => StonePayloadMetaDependency::PkgConfig32,
        _ => return Err(StonePayloadDecodeError::UnknownDependency(i)),
    };
    Ok(result)
}

impl Record for StonePayloadMeta {
    fn decode<R: Read>(mut reader: R) -> Result<Self, StonePayloadDecodeError> {
        let length = reader.read_u32()?;

        let tag = match reader.read_u16()? {
            1 => StonePayloadMetaTag::Name,
            2 => StonePayloadMetaTag::Architecture,
            3 => StonePayloadMetaTag::Version,
            4 => StonePayloadMetaTag::Summary,
            5 => StonePayloadMetaTag::Description,
            6 => StonePayloadMetaTag::Homepage,
            7 => StonePayloadMetaTag::SourceID,
            8 => StonePayloadMetaTag::Depends,
            9 => StonePayloadMetaTag::Provides,
            10 => StonePayloadMetaTag::Conflicts,
            11 => StonePayloadMetaTag::Release,
            12 => StonePayloadMetaTag::License,
            13 => StonePayloadMetaTag::BuildRelease,
            14 => StonePayloadMetaTag::PackageURI,
            15 => StonePayloadMetaTag::PackageHash,
            16 => StonePayloadMetaTag::PackageSize,
            17 => StonePayloadMetaTag::BuildDepends,
            18 => StonePayloadMetaTag::SourceURI,
            19 => StonePayloadMetaTag::SourcePath,
            20 => StonePayloadMetaTag::SourceRef,
            t => return Err(StonePayloadDecodeError::UnknownMetaTag(t)),
        };

        let kind = reader.read_u8()?;
        let _padding = reader.read_array::<1>()?;

        // Remove null terminated byte from string
        let sanitize = |s: String| s.trim_end_matches('\0').to_owned();

        let kind = match kind {
            1 => StonePayloadMetaKind::Int8(reader.read_u8()? as i8),
            2 => StonePayloadMetaKind::Uint8(reader.read_u8()?),
            3 => StonePayloadMetaKind::Int16(reader.read_u16()? as i16),
            4 => StonePayloadMetaKind::Uint16(reader.read_u16()?),
            5 => StonePayloadMetaKind::Int32(reader.read_u32()? as i32),
            6 => StonePayloadMetaKind::Uint32(reader.read_u32()?),
            7 => StonePayloadMetaKind::Int64(reader.read_u64()? as i64),
            8 => StonePayloadMetaKind::Uint64(reader.read_u64()?),
            9 => StonePayloadMetaKind::String(sanitize(reader.read_string(length as u64)?)),
            10 => StonePayloadMetaKind::Dependency(
                // DependencyKind u8 subtracted from length
                decode_dependency(reader.read_u8()?)?,
                sanitize(reader.read_string(length as u64 - 1)?),
            ),
            11 => StonePayloadMetaKind::Provider(
                // DependencyKind u8 subtracted from length
                decode_dependency(reader.read_u8()?)?,
                sanitize(reader.read_string(length as u64 - 1)?),
            ),
            k => return Err(StonePayloadDecodeError::UnknownMetaKind(k)),
        };

        Ok(Self { tag, kind })
    }

    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), StonePayloadEncodeError> {
        let kind = match self.kind {
            StonePayloadMetaKind::Int8(_) => 1,
            StonePayloadMetaKind::Uint8(_) => 2,
            StonePayloadMetaKind::Int16(_) => 3,
            StonePayloadMetaKind::Uint16(_) => 4,
            StonePayloadMetaKind::Int32(_) => 5,
            StonePayloadMetaKind::Uint32(_) => 6,
            StonePayloadMetaKind::Int64(_) => 7,
            StonePayloadMetaKind::Uint64(_) => 8,
            StonePayloadMetaKind::String(_) => 9,
            StonePayloadMetaKind::Dependency(_, _) => 10,
            StonePayloadMetaKind::Provider(_, _) => 11,
        };

        writer.write_u32(self.kind.size() as u32)?;
        writer.write_u16(self.tag as u16)?;
        writer.write_u8(kind)?;
        // Padding
        writer.write_array::<1>([0])?;

        match &self.kind {
            StonePayloadMetaKind::Int8(i) => writer.write_u8(*i as u8)?,
            StonePayloadMetaKind::Uint8(i) => writer.write_u8(*i)?,
            StonePayloadMetaKind::Int16(i) => writer.write_u16(*i as u16)?,
            StonePayloadMetaKind::Uint16(i) => writer.write_u16(*i)?,
            StonePayloadMetaKind::Int32(i) => writer.write_u32(*i as u32)?,
            StonePayloadMetaKind::Uint32(i) => writer.write_u32(*i)?,
            StonePayloadMetaKind::Int64(i) => writer.write_u64(*i as u64)?,
            StonePayloadMetaKind::Uint64(i) => writer.write_u64(*i)?,
            StonePayloadMetaKind::String(s) => {
                writer.write_all(s.as_bytes())?;
                writer.write_u8(b'\0')?;
            }
            StonePayloadMetaKind::Dependency(dep, s) | StonePayloadMetaKind::Provider(dep, s) => {
                writer.write_u8(*dep as u8)?;
                writer.write_all(s.as_bytes())?;
                writer.write_u8(b'\0')?;
            }
        }

        Ok(())
    }

    fn size(&self) -> usize {
        4 + 2 + 1 + 1 + self.kind.size()
    }
}
