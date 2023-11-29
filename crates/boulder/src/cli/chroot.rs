// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fs, io, path::PathBuf};

use boulder::{container, env, Cache, Env};
use clap::Parser;
use thiserror::Error;

use super::Global;

#[derive(Debug, Parser)]
#[command(about = "Chroot into the build environment")]
pub struct Command {
    #[arg(default_value = "./stone.yml", help = "Path to recipe file")]
    recipe: PathBuf,
}

pub fn handle(command: Command, global: Global) -> Result<(), Error> {
    let Command { recipe } = command;
    let Global {
        config_dir,
        cache_dir,
        moss_root,
    } = global;

    if !recipe.exists() {
        return Err(Error::MissingRecipe(recipe));
    }

    let recipe_bytes = fs::read(&recipe)?;
    let recipe = stone_recipe::from_slice(&recipe_bytes)?;

    let env = Env::new(config_dir, cache_dir, moss_root)?;
    let cache = Cache::new(&recipe, env.cache_dir, "/mason")?;
    let rootfs = cache.rootfs().host;

    // Has rootfs been setup?
    if !rootfs.join("usr").exists() {
        return Err(Error::MissingRootFs);
    }

    container::chroot(&recipe, &cache).map_err(Error::Container)?;

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
    #[error("env")]
    Env(#[from] env::Error),
    #[error("stone recipe")]
    StoneRecipe(#[from] stone_recipe::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
