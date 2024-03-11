// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;
use std::path::PathBuf;

use boulder::build::{self, Builder};
use boulder::package::Packager;
use boulder::{container, package, profile, timing, Env, Timing};
use chrono::Local;
use clap::Parser;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(about = "Build ... TODO")]
pub struct Command {
    #[arg(short, long, default_value = "default-x86_64")]
    profile: profile::Id,
    #[arg(
        short,
        long = "compiler-cache",
        help = "Enable compiler caching",
        default_value = "false"
    )]
    ccache: bool,
    #[arg(long, default_value = "false", help = "Update profile repositories before building")]
    update: bool,
    #[arg(short, long, default_value = ".", help = "Directory to store build results")]
    output: PathBuf,
    #[arg(default_value = "./stone.yml", help = "Path to recipe file")]
    recipe: PathBuf,
}

pub fn handle(command: Command, env: Env) -> Result<(), Error> {
    let Command {
        profile,
        output,
        recipe: recipe_path,
        ccache,
        update,
    } = command;

    let mut timing = Timing::default();
    let timer = timing.begin(timing::Kind::Initialize);

    // Resolve dir to dir + stone.yml
    let recipe_path = if recipe_path.is_dir() {
        recipe_path.join("stone.yml")
    } else {
        recipe_path
    };

    if !output.exists() {
        return Err(Error::MissingOutput(output));
    }
    if !recipe_path.exists() {
        return Err(Error::MissingRecipe(recipe_path));
    }

    let builder = Builder::new(&recipe_path, env, profile, ccache)?;
    builder.setup(&mut timing, timer, update)?;

    let paths = &builder.paths;
    let networking = builder.recipe.parsed.options.networking;

    // Build & package from within container
    container::exec::<Error>(paths, networking, || {
        builder.build(&mut timing)?;

        let packager = Packager::new(&builder.paths, &builder.recipe, &builder.macros, &builder.targets)?;
        packager.package(&mut timing)?;

        timing.print_table();

        Ok(())
    })?;

    // Copy artefacts to host recipe dir
    package::sync_artefacts(paths).map_err(Error::SyncArtefacts)?;

    println!(
        "Build finished successfully at {}",
        Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    );

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("output directory does not exist: {0:?}")]
    MissingOutput(PathBuf),
    #[error("recipe file does not exist: {0:?}")]
    MissingRecipe(PathBuf),
    #[error("build recipe")]
    Build(#[from] build::Error),
    #[error("package artifacts")]
    Package(#[from] package::Error),
    #[error("sync artefacts")]
    SyncArtefacts(#[source] io::Error),
    #[error("container")]
    Container(#[from] container::Error),
}
