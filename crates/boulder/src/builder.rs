// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{io, path::Path};

use futures::{stream, StreamExt, TryStreamExt};
use stone_recipe::Recipe;
use thiserror::Error;
use tokio::fs;

use crate::{architecture::BuildTarget, job, macros, paths, pgo, recipe, Env, Job, Macros, Paths};

pub struct Builder {
    pub targets: Vec<Target>,
    pub recipe: Recipe,
    pub paths: Paths,
    pub macros: Macros,
    pub ccache: bool,
}

pub struct Target {
    pub build_target: BuildTarget,
    pub jobs: Vec<Job>,
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

        let build_targets = recipe::build_targets(&recipe);

        if build_targets.is_empty() {
            return Err(Error::NoBuildTargets);
        }

        let targets = stream::iter(&build_targets)
            .then(|build_target| async {
                let stages = pgo::stages(&recipe, *build_target)
                    .map(|stages| stages.into_iter().map(Some).collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![None]);

                let jobs = stream::iter(stages)
                    .then(|stage| Job::new(*build_target, stage, &recipe, &paths, &macros, ccache))
                    .try_collect::<Vec<_>>()
                    .await?;

                Ok(Target {
                    build_target: *build_target,
                    jobs,
                })
            })
            .try_collect::<Vec<_>>()
            .await
            .map_err(Error::Job)?;

        Ok(Self {
            targets,
            recipe,
            paths,
            macros,
            ccache,
        })
    }

    pub fn extra_deps(&self) -> impl Iterator<Item = &str> {
        self.targets.iter().flat_map(|target| {
            target.jobs.iter().flat_map(|job| {
                job.steps
                    .values()
                    .flat_map(|script| script.dependencies.iter().map(String::as_str))
            })
        })
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
