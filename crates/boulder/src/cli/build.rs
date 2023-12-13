// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;
use std::path::PathBuf;

use boulder::builder;
use boulder::Builder;
use boulder::{profile, Env};
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

pub fn handle(command: Command, env: Env) -> Result<(), Error> {
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

    let builder = Builder::new(&recipe, env, profile, ccache)?;
    builder.setup()?;
    builder.build()?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("output directory does not exist: {0:?}")]
    MissingOutput(PathBuf),
    #[error("recipe file does not exist: {0:?}")]
    MissingRecipe(PathBuf),
    #[error("builder")]
    Builder(#[from] builder::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
