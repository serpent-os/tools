// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{io, path::Path};

use futures::{stream, StreamExt, TryStreamExt};
use stone_recipe::Recipe;
use thiserror::Error;
use tokio::fs;

use crate::{
    architecture::{self, BuildTarget},
    job, macros, paths, Env, Job, Macros, Paths,
};

pub struct Builder {
    pub jobs: Vec<Job>,
    pub recipe: Recipe,
    pub paths: Paths,
    pub macros: Macros,
    pub ccache: bool,
}

impl Builder {
    pub async fn new(recipe_path: &Path, env: &Env, ccache: bool) -> Result<Self, Error> {
        let recipe_bytes = fs::read(recipe_path).await?;
        let recipe = stone_recipe::from_slice(&recipe_bytes)?;

        let macros = Macros::load(env).await?;

        let paths = Paths::new(
            paths::Id::new(&recipe),
            recipe_path,
            &env.cache_dir,
            "/mason",
        )
        .await?;

        let build_targets = build_targets(&recipe);

        if build_targets.is_empty() {
            return Err(Error::NoBuildTargets);
        }

        let jobs = stream::iter(build_targets)
            .then(|target| Job::new(target, &recipe, &paths, &macros, ccache))
            .try_collect::<Vec<_>>()
            .await?;

        Ok(Self {
            jobs,
            recipe,
            paths,
            macros,
            ccache,
        })
    }
}

fn build_targets(recipe: &Recipe) -> Vec<BuildTarget> {
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("no supported build targets for recipe")]
    NoBuildTargets,
    #[error("macros")]
    Macros(#[from] macros::Error),
    #[error("job")]
    Job(#[from] job::Error),
    #[error("stone recipe")]
    StoneRecipe(#[from] stone_recipe::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
