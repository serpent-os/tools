// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::str::FromStr;

use derive_more::Display;
use stone::payload;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum Kind {
    /// Name based dependency
    #[strum(serialize = "name")]
    PackageName,

    /// Shared library (soname)
    #[strum(serialize = "soname")]
    SharedLibary,

    /// Exported pkg-config provider
    PkgConfig,

    /// PT_INTERP, or specialist shell support
    Interpreter,

    /// CMake module dependency
    CMake,

    /// Python dependency (unused)
    Python,

    /// Executable in /usr/bin
    Binary,

    /// Executable in /usr/sbin
    #[strum(serialize = "sysbinary")]
    SystemBinary,

    /// Exported 32-bit pkgconfig provider
    PkgConfig32,
}

/// Convert payload dependency types to our internal representation
impl From<payload::meta::Dependency> for Kind {
    fn from(dependency: payload::meta::Dependency) -> Self {
        match dependency {
            payload::meta::Dependency::PackageName => Kind::PackageName,
            payload::meta::Dependency::SharedLibary => Kind::SharedLibary,
            payload::meta::Dependency::PkgConfig => Kind::PkgConfig,
            payload::meta::Dependency::Interpreter => Kind::Interpreter,
            payload::meta::Dependency::CMake => Kind::CMake,
            payload::meta::Dependency::Python => Kind::Python,
            payload::meta::Dependency::Binary => Kind::Binary,
            payload::meta::Dependency::SystemBinary => Kind::SystemBinary,
            payload::meta::Dependency::PkgConfig32 => Kind::PkgConfig32,
        }
    }
}

impl From<Kind> for payload::meta::Dependency {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::PackageName => Self::PackageName,
            Kind::SharedLibary => Self::SharedLibary,
            Kind::PkgConfig => Self::PkgConfig,
            Kind::Interpreter => Self::Interpreter,
            Kind::CMake => Self::CMake,
            Kind::Python => Self::Python,
            Kind::Binary => Self::Binary,
            Kind::SystemBinary => Self::SystemBinary,
            Kind::PkgConfig32 => Self::PkgConfig32,
        }
    }
}

/// A Dependency in moss is simplistic in that it only contains
/// a target and a Kind, ie. `pkgconfig(zlib)`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
#[display(fmt = "{kind}({name})")]
pub struct Dependency {
    /// Tag for the table-type of dependency
    pub kind: Kind,

    /// Bare target
    pub name: String,
}

impl Dependency {
    pub fn from_name(name: &str) -> Result<Self, ParseError> {
        if name.contains('(') {
            Dependency::from_str(name)
        } else {
            Ok(Dependency {
                kind: Kind::PackageName,
                name: name.to_owned(),
            })
        }
    }
}

impl PartialOrd for Dependency {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dependency {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl FromStr for Dependency {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, name) = parse(s)?;

        Ok(Self { kind, name })
    }
}

impl<'a> TryFrom<&'a str> for Dependency {
    type Error = ParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Display)]
#[display(fmt = "{kind}({name})")]
pub struct Provider {
    pub kind: Kind,
    pub name: String,
}

impl Provider {
    pub fn from_name(name: &str) -> Result<Self, ParseError> {
        if name.contains('(') {
            Provider::from_str(name)
        } else {
            Ok(Provider {
                kind: Kind::PackageName,
                name: name.to_owned(),
            })
        }
    }
}

impl PartialOrd for Provider {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Provider {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl FromStr for Provider {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, name) = parse(s)?;

        Ok(Self { kind, name })
    }
}

impl<'a> TryFrom<&'a str> for Provider {
    type Error = ParseError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

fn parse(s: &str) -> Result<(Kind, String), ParseError> {
    let (kind, rest) = s.split_once('(').ok_or(ParseError(s.to_string()))?;

    if !rest.ends_with(')') {
        return Err(ParseError(s.to_string()));
    }

    let kind = kind.parse::<Kind>().map_err(|e| ParseError(e.to_string()))?;
    // Safe since we checked `ends_with(')')`
    let name = rest[0..rest.len() - 1].to_string();

    Ok((kind, name))
}

#[derive(Debug, Error)]
#[error("Invalid dependency type: {0}")]
pub struct ParseError(String);
