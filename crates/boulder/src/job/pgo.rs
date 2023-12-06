// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use stone_recipe::{tuning::Toolchain, Recipe};

use super::build_target_definition;
use crate::architecture::BuildTarget;

pub fn stages(target: BuildTarget, recipe: &Recipe) -> Vec<Stage> {
    let build = build_target_definition(target, recipe);

    build
        .workload
        .is_some()
        .then(|| {
            let mut stages = vec![Stage::One];

            if matches!(recipe.options.toolchain, Toolchain::Llvm) && recipe.options.cspgo {
                stages.push(Stage::Two);
            }

            stages.push(Stage::Use);

            stages
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, strum::Display)]
pub enum Stage {
    #[strum(serialize = "stage1")]
    One,
    #[strum(serialize = "stage1")]
    Two,
    #[strum(serialize = "use")]
    Use,
}
