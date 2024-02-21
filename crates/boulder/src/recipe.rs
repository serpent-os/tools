// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fs, io, path::PathBuf};

use thiserror::Error;

use crate::architecture::{self, BuildTarget};

pub type Parsed = stone_recipe::Recipe;

#[derive(Debug)]
pub struct Recipe {
    pub path: PathBuf,
    pub source: String,
    pub parsed: Parsed,
}

impl Recipe {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let source = fs::read_to_string(&path)?;
        let parsed = stone_recipe::from_str(&source)?;

        Ok(Self { path, source, parsed })
    }

    pub fn build_targets(&self) -> Vec<BuildTarget> {
        let host = architecture::host();
        let host_string = host.to_string();

        if self.parsed.architectures.is_empty() {
            let mut targets = vec![BuildTarget::Native(host)];

            if self.parsed.emul32 {
                targets.push(BuildTarget::Emul32(host));
            }

            targets
        } else {
            let mut targets = vec![];

            if self.parsed.architectures.contains(&host_string) || self.parsed.architectures.contains(&"native".into())
            {
                targets.push(BuildTarget::Native(host));
            }

            let emul32 = BuildTarget::Emul32(host);
            let emul32_string = emul32.to_string();

            if self.parsed.architectures.contains(&emul32_string)
                || self.parsed.architectures.contains(&"emul32".into())
            {
                targets.push(emul32);
            }

            targets
        }
    }

    pub fn build_target_profile_key(&self, target: BuildTarget) -> Option<String> {
        let target_string = target.to_string();

        if self.parsed.profiles.iter().any(|kv| kv.key == target_string) {
            Some(target_string)
        } else if target.emul32() && self.parsed.profiles.iter().any(|kv| &kv.key == "emul32") {
            Some("emul32".to_string())
        } else {
            None
        }
    }

    pub fn build_target_definition(&self, target: BuildTarget) -> &stone_recipe::Build {
        let key = self.build_target_profile_key(target);

        if let Some(profile) = self.parsed.profiles.iter().find(|kv| Some(&kv.key) == key.as_ref()) {
            &profile.value
        } else {
            &self.parsed.build
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("load recipe")]
    Load(#[from] io::Error),
    #[error("decode recipe")]
    Decode(#[from] stone_recipe::Error),
}
