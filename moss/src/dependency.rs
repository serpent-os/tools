// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fmt, str::FromStr};

use stone::payload;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    PackageName,
    SharedLibary,
    PkgConfig,
    Interpreter,
    CMake,
    Python,
    Binary,
    SystemBinary,
    PkgConfig32,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::PackageName => write!(f, "name"),
            Kind::SharedLibary => write!(f, "soname"),
            Kind::PkgConfig => write!(f, "pkgconfig"),
            Kind::Interpreter => write!(f, "interpreter"),
            Kind::CMake => write!(f, "cmake"),
            Kind::Python => write!(f, "python"),
            Kind::Binary => write!(f, "binary"),
            Kind::SystemBinary => write!(f, "sysbinary"),
            Kind::PkgConfig32 => write!(f, "pkgconfig32"),
        }
    }
}

impl FromStr for Kind {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "name" => Kind::PackageName,
            "soname" => Kind::SharedLibary,
            "pkgconfig" => Kind::PkgConfig,
            "interpreter" => Kind::Interpreter,
            "cmake" => Kind::CMake,
            "python" => Kind::Python,
            "binary" => Kind::Binary,
            "sysbinary" => Kind::SystemBinary,
            "pkgconfig32" => Kind::PkgConfig32,
            _ => return Err(ParseError(s.to_string())),
        })
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub kind: Kind,
    pub name: String,
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.kind, self.name)
    }
}

impl FromStr for Dependency {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, name) = parse(s)?;

        Ok(Self { kind, name })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    pub kind: Kind,
    pub name: String,
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.kind, self.name)
    }
}

impl FromStr for Provider {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, name) = parse(s)?;

        Ok(Self { kind, name })
    }
}

fn parse(s: &str) -> Result<(Kind, String), ParseError> {
    let (kind, rest) = s.split_once('(').ok_or(ParseError(s.to_string()))?;

    if !rest.ends_with(')') {
        return Err(ParseError(s.to_string()));
    }

    let kind = kind.parse()?;
    let name = rest.trim_matches(')').to_string();

    Ok((kind, name))
}

#[derive(Debug, Error)]
#[error("Invalid dependency type: {0}")]
pub struct ParseError(String);
