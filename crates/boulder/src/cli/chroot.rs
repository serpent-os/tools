// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{io, path::PathBuf};

use boulder::{container, paths, Env, Paths, Runtime};
use clap::Parser;
use thiserror::Error;
use tokio::fs;

#[derive(Debug, Parser)]
#[command(about = "Chroot into the build environment")]
pub struct Command {
    #[arg(default_value = "./stone.yml", help = "Path to recipe file")]
    recipe: PathBuf,
}

pub fn handle(command: Command, rt: Runtime, env: Env) -> Result<(), Error> {
    let Command {
        recipe: recipe_path,
    } = command;

    if !recipe_path.exists() {
        return Err(Error::MissingRecipe(recipe_path));
    }

    let recipe_bytes = rt.block_on(fs::read(&recipe_path))?;
    let recipe = stone_recipe::from_slice(&recipe_bytes)?;

    let paths = rt.block_on(Paths::new(
        paths::Id::new(&recipe),
        &recipe_path,
        &env.cache_dir,
        "/mason",
    ))?;

    let rootfs = paths.rootfs().host;

    // Has rootfs been setup?
    if !rootfs.join("usr").exists() {
        return Err(Error::MissingRootFs);
    }

    rt.destroy();

    container::chroot(&paths, recipe.options.networking).map_err(Error::Container)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("recipe file does not exist: {0:?}")]
    MissingRecipe(PathBuf),
    #[error("build root doesn't exist, make sure to run build first")]
    MissingRootFs,
    #[error("container")]
    Container(container::Error),
    #[error("stone recipe")]
    StoneRecipe(#[from] stone_recipe::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
