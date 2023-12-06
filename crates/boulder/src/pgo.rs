// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use stone_recipe::{tuning::Toolchain, Recipe};

use crate::architecture::BuildTarget;
use crate::recipe;

pub fn stages(recipe: &Recipe, target: BuildTarget) -> Option<Vec<Stage>> {
    let build = recipe::build_target_definition(recipe, target);

    build.workload.is_some().then(|| {
        let mut stages = vec![Stage::One];

        if matches!(recipe.options.toolchain, Toolchain::Llvm) && recipe.options.cspgo {
            stages.push(Stage::Two);
        }

        stages.push(Stage::Use);

        stages
    })
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
