// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use serde::Deserialize;

use crate::{sequence_of_key_value, single_as_sequence, KeyValue};

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

#[derive(Debug, Clone, Deserialize)]
pub struct TuningFlag {
    #[serde(flatten)]
    root: CompilerFlags,
    #[serde(default)]
    gnu: CompilerFlags,
    #[serde(default)]
    llvm: CompilerFlags,
}

impl TuningFlag {
    pub fn get(&self, flag: CompilerFlag, toolchain: Toolchain) -> Option<&str> {
        match toolchain {
            Toolchain::Llvm => self.llvm.get(flag),
            Toolchain::Gnu => self.gnu.get(flag),
        }
        .and(self.root.get(flag))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CompilerFlag {
    C,
    Cxx,
    D,
    Ld,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CompilerFlags {
    c: Option<String>,
    cxx: Option<String>,
    d: Option<String>,
    ld: Option<String>,
}

impl CompilerFlags {
    fn get(&self, flag: CompilerFlag) -> Option<&str> {
        match flag {
            CompilerFlag::C => self.c.as_deref(),
            CompilerFlag::Cxx => self.cxx.as_deref(),
            CompilerFlag::D => self.d.as_deref(),
            CompilerFlag::Ld => self.ld.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Toolchain {
    #[default]
    Llvm,
    Gnu,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TuningOption {
    #[serde(default, deserialize_with = "single_as_sequence")]
    pub enabled: Vec<String>,
    #[serde(default, deserialize_with = "single_as_sequence")]
    pub disabled: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TuningGroup {
    #[serde(flatten, default)]
    pub root: TuningOption,
    pub default: Option<String>,
    #[serde(
        default,
        rename = "options",
        deserialize_with = "sequence_of_key_value"
    )]
    pub choices: Vec<KeyValue<TuningOption>>,
}
