// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;
use std::path::PathBuf;

use boulder::builder;
use boulder::upstream;
use boulder::Builder;
use boulder::{container, profile, root, Env, Runtime};
use clap::Parser;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(about = "Build ... TODO")]
pub struct Command {
    #[arg(short, long)]
    profile: profile::Id,
    #[arg(
        short,
        long = "compiler-cache",
        help = "Enable compiler caching",
        default_value = "false"
    )]
    ccache: bool,
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

pub fn handle(command: Command, rt: Runtime, env: Env) -> Result<(), Error> {
    let Command {
        profile,
        output,
        recipe,
        ccache,
    } = command;

    if !output.exists() {
        return Err(Error::MissingOutput(output));
    }
    if !recipe.exists() {
        return Err(Error::MissingRecipe(recipe));
    }

    let builder = rt.block_on(Builder::new(&recipe, &env, ccache))?;

    let profiles = rt.block_on(profile::Manager::new(&env));
    let repos = profiles.repositories(&profile)?.clone();

    rt.block_on(root::populate(&env, &builder, repos))?;

    rt.block_on(upstream::sync(&builder.recipe, &builder.paths))?;

    // Destroy async runtime since we will
    // transition into the container
    rt.destroy();

    container::exec(builder).map_err(Error::Container)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("output directory does not exist: {0:?}")]
    MissingOutput(PathBuf),
    #[error("recipe file does not exist: {0:?}")]
    MissingRecipe(PathBuf),
    #[error("profile")]
    Profile(#[from] profile::Error),
    #[error("root")]
    Root(#[from] root::Error),
    #[error("upstream")]
    Upstream(#[from] upstream::Error),
    #[error("builder")]
    Builder(#[from] builder::Error),
    #[error("io")]
    Io(#[from] io::Error),
    #[error("container")]
    Container(#[source] container::Error),
}
