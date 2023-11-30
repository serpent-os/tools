// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use boulder::{container, job, Env, Job, Runtime};
use clap::Parser;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(about = "Chroot into the build environment")]
pub struct Command {
    #[arg(default_value = "./stone.yml", help = "Path to recipe file")]
    recipe: PathBuf,
}

pub fn handle(command: Command, rt: Runtime, env: Env) -> Result<(), Error> {
    let Command { recipe } = command;

    if !recipe.exists() {
        return Err(Error::MissingRecipe(recipe));
    }

    let job = rt.block_on(Job::new(&recipe, &env))?;

    let rootfs = job.paths.rootfs().host;

    // Has rootfs been setup?
    if !rootfs.join("usr").exists() {
        return Err(Error::MissingRootFs);
    }

    container::chroot(&job).map_err(Error::Container)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("recipe file does not exist: {0:?}")]
    MissingRecipe(PathBuf),
    #[error("build root doesn't exist, make sure to run build first")]
    MissingRootFs,
    #[error("container")]
    Container(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("job")]
    Job(#[from] job::Error),
}
