// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::{Path, PathBuf};

use stone_recipe::{tuning::Toolchain, Recipe};

use crate::architecture::BuildTarget;

#[derive(Debug, Clone)]
pub struct Pgo {
    pub stages: Vec<Stage>,
    pub workload: String,
    pub build_dir: PathBuf,
}

impl Pgo {
    pub fn new(target: BuildTarget, recipe: &Recipe, build_dir: &Path) -> Option<Self> {
        let mut build = &recipe.build;

        let target_string = target.to_string();

        if let Some(profile) = recipe
            .profiles
            .iter()
            .find(|profile| profile.key == target_string)
        {
            build = &profile.value;
        } else if target.emul32() {
            if let Some(profile) = recipe
                .profiles
                .iter()
                .find(|profile| &profile.key == "emul32")
            {
                build = &profile.value;
            }
        }

        build.workload.clone().map(|workload| {
            let mut stages = vec![Stage::One];

            if matches!(recipe.options.toolchain, Toolchain::Llvm) && recipe.options.cspgo {
                stages.push(Stage::Two);
            }

            stages.push(Stage::Use);

            Self {
                stages,
                workload,
                build_dir: format!("{}-pgo", build_dir.display()).into(),
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    One,
    Two,
    Use,
}
