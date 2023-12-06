// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use stone_recipe::Recipe;

use crate::architecture::{self, BuildTarget};

pub fn build_targets(recipe: &Recipe) -> Vec<BuildTarget> {
    let host = architecture::host();
    let host_string = host.to_string();

    if recipe.architectures.is_empty() {
        let mut targets = vec![BuildTarget::Native(host)];

        if recipe.emul32 {
            targets.push(BuildTarget::Emul32(host));
        }

        targets
    } else {
        let mut targets = vec![];

        if recipe.architectures.contains(&host_string)
            || recipe.architectures.contains(&"native".into())
        {
            targets.push(BuildTarget::Native(host));
        }

        let emul32 = BuildTarget::Emul32(host);
        let emul32_string = emul32.to_string();

        if recipe.architectures.contains(&emul32_string)
            || recipe.architectures.contains(&"emul32".into())
        {
            targets.push(emul32);
        }

        targets
    }
}

pub fn build_target_definition(recipe: &Recipe, target: BuildTarget) -> &stone_recipe::Build {
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

    build
}
