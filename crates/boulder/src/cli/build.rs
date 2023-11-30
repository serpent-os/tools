// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::fs;
use std::io;
use std::path::PathBuf;

use boulder::upstream;
use boulder::{container, env, profile, root, Cache, Env, Runtime};
use clap::Parser;
use thiserror::Error;

use super::Global;

#[derive(Debug, Parser)]
#[command(about = "Build ... TODO")]
pub struct Command {
    #[arg(short, long)]
    profile: profile::Id,
    #[arg(
        short,
        long,
        default_value = ".",
        help = "Directory to store build results"
    )]
    output: PathBuf,
    #[arg(default_value = "./stone.yml", help = "Path to recipe file")]
    recipe: PathBuf,
}

pub fn handle(command: Command, global: Global) -> Result<(), Error> {
    let Command {
        profile,
        output,
        recipe: recipe_path,
    } = command;
    let Global {
        moss_root,
        config_dir,
        cache_dir,
    } = global;

    if !output.exists() {
        return Err(Error::MissingOutput(output));
    }
    if !recipe_path.exists() {
        return Err(Error::MissingRecipe(recipe_path));
    }

    let recipe_bytes = fs::read(&recipe_path)?;
    let recipe = stone_recipe::from_slice(&recipe_bytes)?;

    let rt = Runtime::new()?;
    let env = Env::new(config_dir, cache_dir, moss_root)?;
    let cache = Cache::new(&recipe, recipe_path, &env.cache_dir, "/mason")?;

    let profiles = rt.block_on(profile::Manager::new(&env));
    let repos = profiles.repositories(&profile)?.clone();

    // TODO: ccache config
    rt.block_on(root::populate(&env, &cache, repos, &recipe, false))?;

    rt.block_on(upstream::sync(&recipe, &cache))?;

    // Drop async runtime
    drop(rt);

    // TODO: Exec build scripts
    container::chroot(&recipe, &cache).map_err(Error::Container)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("output directory does not exist: {0:?}")]
    MissingOutput(PathBuf),
    #[error("recipe file does not exist: {0:?}")]
    MissingRecipe(PathBuf),
    #[error("container")]
    Container(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("env")]
    Env(#[from] env::Error),
    #[error("profile")]
    Profile(#[from] profile::Error),
    #[error("root")]
    Root(#[from] root::Error),
    #[error("upstream")]
    Upstream(#[from] upstream::Error),
    #[error("stone recipe")]
    StoneRecipe(#[from] stone_recipe::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
