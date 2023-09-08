use std::{
    fmt::Display,
    io::{self, Read},
};

use thiserror::Error;

use crate::ReadExt;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    // Payload has no compression
    None = 1,
    // Payload uses ZSTD compression
    Zstd = 2,
}

#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub stored_size: u64,
    pub plain_size: u64,
    pub checksum: [u8; 8],
    pub num_records: usize,
    pub version: u16,
    pub kind: Kind,
    pub compression: Compression,
}

impl Header {
    pub fn decode<R: Read>(mut reader: R) -> Result<Self, DecodeError> {
        let stored_size = reader.read_u64()?;
        let plain_size = reader.read_u64()?;
        let checksum = reader.read_array()?;
        let num_records = reader.read_u32()? as usize;
        let version = reader.read_u16()?;

        let kind = match reader.read_u8()? {
            1 => Kind::Meta,
            2 => Kind::Content,
            3 => Kind::Layout,
            4 => Kind::Index,
            5 => Kind::Attributes,
            6 => Kind::Dumb,
            k => return Err(DecodeError::UnknownKind(k)),
        };

        let compression = match reader.read_u8()? {
            1 => Compression::None,
            2 => Compression::Zstd,
            d => return Err(DecodeError::UnknownCompression(d)),
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
}

pub trait Record: Sized {
    fn decode<R: Read>(reader: R) -> Result<Self, DecodeError>;
}

pub fn decode_records<T: Record, R: Read>(
    mut reader: R,
    num_records: usize,
) -> Result<Vec<T>, DecodeError> {
    let mut records = Vec::with_capacity(num_records);

    for _ in 0..num_records {
        records.push(T::decode(&mut reader)?);
    }

    Ok(records)
}

#[derive(Debug, Clone, Copy)]
pub struct Index {
    pub start: u64,
    pub end: u64,
    pub digest: u128,
}

impl Record for Index {
    fn decode<R: Read>(mut reader: R) -> Result<Self, DecodeError> {
        let start = reader.read_u64()?;
        let end = reader.read_u64()?;
        let digest = reader.read_u128()?;

        Ok(Self { start, end, digest })
    }
}

#[derive(Debug)]
pub struct Attribute {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

impl Record for Attribute {
    fn decode<R: Read>(mut reader: R) -> Result<Self, DecodeError> {
        let key_length = reader.read_u64()?;
        let value_length = reader.read_u64()?;

        let key = reader.read_vec(key_length as usize)?;
        let value = reader.read_vec(value_length as usize)?;

        Ok(Self { key, value })
    }
}

///
/// Layout entries record their target file type so they can be rebuilt on
/// the target installation.
///
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

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyKind {
    /// Just the plain name of a package
    PackageName = 0,

    /// A soname based dependency
    SharedLibary,

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
    SystemBinary,

    /// An emul32-compatible pkgconfig .pc dependency (lib32/*.pc)
    PkgConfig32,
}

///
/// Override display for `pkgconfig32(name)` style strings
///
impl Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyKind::PackageName => write!(f, "name"),
            DependencyKind::SharedLibary => write!(f, "soname"),
            DependencyKind::PkgConfig => write!(f, "pkgconfig"),
            DependencyKind::Interpreter => write!(f, "interpreter"),
            DependencyKind::CMake => write!(f, "cmake"),
            DependencyKind::Python => write!(f, "python"),
            DependencyKind::Binary => write!(f, "binary"),
            DependencyKind::SystemBinary => write!(f, "sysbinary"),
            DependencyKind::PkgConfig32 => write!(f, "pkgconfig32"),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaKind {
    Int8(i8),
    Uint8(u8),
    Int16(i16),
    Uint16(u16),
    Int32(i32),
    Uint32(u32),
    Int64(i64),
    Uint64(u64),
    String(String),
    Dependency(DependencyKind, String),
    Provider(DependencyKind, String),
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaTag {
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

// TODO: Strong types these fields
#[derive(Debug)]
pub struct Meta {
    pub tag: MetaTag,
    pub kind: MetaKind,
}

///
/// Helper to decode a dependency's encoded kind
///
fn decode_dependency(i: u8) -> Result<DependencyKind, DecodeError> {
    let result = match i {
        0 => DependencyKind::PackageName,
        1 => DependencyKind::SharedLibary,
        2 => DependencyKind::PkgConfig,
        3 => DependencyKind::Interpreter,
        4 => DependencyKind::CMake,
        5 => DependencyKind::Python,
        6 => DependencyKind::Binary,
        7 => DependencyKind::SystemBinary,
        8 => DependencyKind::PkgConfig32,
        _ => return Err(DecodeError::UnknownDependency(i)),
    };
    Ok(result)
}

impl Record for Meta {
    fn decode<R: Read>(mut reader: R) -> Result<Self, DecodeError> {
        let length = reader.read_u32()?;

        let tag = match reader.read_u16()? {
            1 => MetaTag::Name,
            2 => MetaTag::Architecture,
            3 => MetaTag::Version,
            4 => MetaTag::Summary,
            5 => MetaTag::Description,
            6 => MetaTag::Homepage,
            7 => MetaTag::SourceID,
            8 => MetaTag::Depends,
            9 => MetaTag::Provides,
            10 => MetaTag::Conflicts,
            11 => MetaTag::Release,
            12 => MetaTag::License,
            13 => MetaTag::BuildRelease,
            14 => MetaTag::PackageURI,
            15 => MetaTag::PackageHash,
            16 => MetaTag::PackageSize,
            17 => MetaTag::BuildDepends,
            18 => MetaTag::SourceURI,
            19 => MetaTag::SourcePath,
            20 => MetaTag::SourceRef,
            t => return Err(DecodeError::UnknownMetaTag(t)),
        };

        let kind = reader.read_u8()?;
        let _padding = reader.read_array::<1>()?;

        let kind = match kind {
            1 => MetaKind::Int8(reader.read_u8()? as i8),
            2 => MetaKind::Uint8(reader.read_u8()?),
            3 => MetaKind::Int16(reader.read_u16()? as i16),
            4 => MetaKind::Uint16(reader.read_u16()?),
            5 => MetaKind::Int32(reader.read_u32()? as i32),
            6 => MetaKind::Uint32(reader.read_u32()?),
            7 => MetaKind::Int64(reader.read_u64()? as i64),
            8 => MetaKind::Uint64(reader.read_u64()?),
            9 => MetaKind::String(reader.read_string(length as u64)?),
            10 => MetaKind::Dependency(
                /* DependencyKind u8 subtracted from length  */
                decode_dependency(reader.read_u8()?)?,
                reader.read_string(length as u64 - 1)?,
            ),
            11 => MetaKind::Provider(
                /* DependencyKind u8 subtracted from length  */
                decode_dependency(reader.read_u8()?)?,
                reader.read_string(length as u64 - 1)?,
            ),
            k => return Err(DecodeError::UnknownMetaKind(k)),
        };

        Ok(Self { tag, kind })
    }
}

#[derive(Debug, Error)]
pub enum DecodeError {
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
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}
