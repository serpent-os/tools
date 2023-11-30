// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::HashMap, hash::Hash, path::PathBuf};

use serde::Deserialize;
use thiserror::Error;
use url::Url;

pub fn from_slice(bytes: &[u8]) -> Result<Recipe, serde_yaml::Error> {
    serde_yaml::from_slice(bytes)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Recipe {
    #[serde(flatten)]
    pub source: Source,
    #[serde(flatten)]
    pub build: Build,
    #[serde(flatten)]
    pub package: Package,
    #[serde(flatten)]
    pub options: Options,
    #[serde(default, deserialize_with = "sequence_of_key_value")]
    pub profiles: Vec<KeyValue<Build>>,
    #[serde(
        default,
        rename = "packages",
        deserialize_with = "sequence_of_key_value"
    )]
    pub sub_packages: Vec<KeyValue<Package>>,
    #[serde(default)]
    pub upstreams: Vec<Upstream>,
    #[serde(default)]
    pub architectures: Vec<String>,
    #[serde(default)]
    pub tuning: Vec<KeyValue<Tuning>>,
    #[serde(default)]
    pub emul32: bool,
}

#[derive(Debug, Clone)]
pub struct KeyValue<T> {
    pub key: String,
    pub value: T,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Source {
    pub name: String,
    pub version: String,
    pub release: u64,
    pub homepage: String,
    #[serde(deserialize_with = "single_as_sequence")]
    pub license: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Build {
    pub setup: Option<String>,
    pub build: Option<String>,
    pub install: Option<String>,
    pub check: Option<String>,
    pub workload: Option<String>,
    pub environment: Option<String>,
    #[serde(default, rename = "builddeps")]
    pub build_deps: Vec<String>,
    #[serde(default, rename = "checkdeps")]
    pub check_deps: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Options {
    #[serde(default)]
    pub toolchain: Toolchain,
    #[serde(default)]
    pub cspgo: bool,
    #[serde(default)]
    pub samplepgo: bool,
    #[serde(default = "default_true")]
    pub strip: bool,
    #[serde(default)]
    pub networking: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    pub summary: Option<String>,
    pub description: Option<String>,
    #[serde(default, rename = "rundeps")]
    pub run_deps: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Toolchain {
    #[default]
    Llvm,
    Gnu,
}

#[derive(Debug, Clone)]
pub enum Upstream {
    Plain {
        uri: Url,
        hash: String,
        rename: Option<String>,
        strip_dirs: u8,
        unpack: bool,
        unpack_dir: PathBuf,
    },
    Git {
        uri: Url,
        ref_id: String,
        clone_dir: Option<PathBuf>,
        staging: bool,
    },
}

impl<'de> Deserialize<'de> for Upstream {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        fn default_unpack_dir() -> PathBuf {
            ".".into()
        }

        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum Inner {
            Plain {
                hash: String,
                rename: Option<String>,
                #[serde(default, rename = "stripdirs")]
                strip_dirs: u8,
                #[serde(default = "default_true")]
                unpack: bool,
                #[serde(default = "default_unpack_dir", rename = "unpackdir")]
                unpack_dir: PathBuf,
            },
            Git {
                #[serde(rename = "ref")]
                ref_id: String,
                #[serde(rename = "clonedir")]
                clone_dir: Option<PathBuf>,
                #[serde(default = "default_true")]
                staging: bool,
            },
        }

        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum Outer {
            String(String),
            Inner(Inner),
        }

        #[derive(Debug, Deserialize, PartialEq, Eq, Hash)]
        #[serde(try_from = "&str")]
        enum Uri {
            Plain(Url),
            Git(Url),
        }

        impl<'a> TryFrom<&'a str> for Uri {
            type Error = UriParseError;

            fn try_from(s: &'a str) -> Result<Self, Self::Error> {
                match s.split_once("git|") {
                    Some((_, uri)) => Ok(Uri::Git(uri.parse()?)),
                    None => Ok(Uri::Plain(s.parse()?)),
                }
            }
        }

        #[derive(Debug, Error)]
        #[error("invalid uri: {0}")]
        struct UriParseError(#[from] url::ParseError);

        let raw_map = HashMap::<Uri, Outer>::deserialize(deserializer)?;

        match raw_map.into_iter().next() {
            Some((Uri::Plain(uri), Outer::String(hash))) => Ok(Upstream::Plain {
                uri,
                hash,
                rename: None,
                strip_dirs: 0,
                unpack: default_true(),
                unpack_dir: default_unpack_dir(),
            }),
            Some((Uri::Git(uri), Outer::String(ref_id))) => Ok(Upstream::Git {
                uri,
                ref_id,
                clone_dir: None,
                staging: default_true(),
            }),
            Some((
                Uri::Plain(uri),
                Outer::Inner(Inner::Plain {
                    hash,
                    rename,
                    strip_dirs,
                    unpack,
                    unpack_dir,
                }),
            )) => Ok(Upstream::Plain {
                uri,
                hash,
                rename,
                strip_dirs,
                unpack,
                unpack_dir,
            }),
            Some((
                Uri::Git(uri),
                Outer::Inner(Inner::Git {
                    ref_id,
                    clone_dir,
                    staging,
                }),
            )) => Ok(Upstream::Git {
                uri,
                ref_id,
                clone_dir,
                staging,
            }),
            Some((Uri::Plain(_), Outer::Inner(Inner::Git { .. }))) => Err(
                serde::de::Error::custom("found git payload but missing 'git|' prefixed URI"),
            ),
            Some((Uri::Git(_), Outer::Inner(Inner::Plain { .. }))) => Err(
                serde::de::Error::custom("found git URI but plain payload fields"),
            ),
            // unreachable?
            None => Err(serde::de::Error::custom("missing upstream entry")),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Tuning {
    Enable,
    Disable,
    Config(String),
}

impl<'de> Deserialize<'de> for KeyValue<Tuning> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum Inner {
            Bool(bool),
            Config(String),
        }

        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum Outer {
            Key(String),
            KeyValue(HashMap<String, Inner>),
        }

        match Outer::deserialize(deserializer)? {
            Outer::Key(key) => Ok(KeyValue {
                key,
                value: Tuning::Enable,
            }),
            Outer::KeyValue(map) => match map.into_iter().next() {
                Some((key, Inner::Bool(true))) => Ok(KeyValue {
                    key,
                    value: Tuning::Enable,
                }),
                Some((key, Inner::Bool(false))) => Ok(KeyValue {
                    key,
                    value: Tuning::Disable,
                }),
                Some((key, Inner::Config(config))) => Ok(KeyValue {
                    key,
                    value: Tuning::Config(config),
                }),
                // unreachable?
                None => Err(serde::de::Error::custom("missing tuning entry")),
            },
        }
    }
}

fn default_true() -> bool {
    true
}

/// Deserialize a single value or sequence of values as a vec
fn single_as_sequence<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::de::Deserializer<'de>,
{
    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    enum Value<T> {
        Single(T),
        Sequence(Vec<T>),
    }

    match Value::deserialize(deserializer)? {
        Value::Single(value) => Ok(vec![value]),
        Value::Sequence(sequence) => Ok(sequence),
    }
}

/// Deserialize a sequence of single entry maps as a vec of [`KeyValue`]
fn sequence_of_key_value<'de, T, D>(deserializer: D) -> Result<Vec<KeyValue<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::de::Deserializer<'de>,
{
    let sequence = Vec::<HashMap<String, T>>::deserialize(deserializer)?;

    Ok(sequence.into_iter().fold(vec![], |acc, next| {
        acc.into_iter()
            .chain(
                next.into_iter()
                    .next()
                    .map(|(key, value)| KeyValue { key, value }),
            )
            .collect()
    }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn deserialize() {
        let inputs = [
            &include_bytes!("../../../test/llvm-stone.yml")[..],
            &include_bytes!("../../../test/boulder-stone.yml")[..],
        ];

        for input in inputs {
            let recipe = from_slice(input).unwrap();
            dbg!(&recipe);
        }
    }
}
