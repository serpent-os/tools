// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fs, io, path::PathBuf, process};

use boulder::{
    architecture::{self, BuildTarget},
    builder, container, job, macros, paths, Env, Macros, Paths,
};
use clap::Parser;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(about = "Chroot into the build environment")]
pub struct Command {
    #[arg(default_value = "./stone.yml", help = "Path to recipe file")]
    recipe: PathBuf,
}

pub fn handle(command: Command, env: Env) -> Result<(), Error> {
    let Command {
        recipe: recipe_path,
    } = command;

    if !recipe_path.exists() {
        return Err(Error::MissingRecipe(recipe_path));
    }

    let recipe_bytes = fs::read(&recipe_path)?;
    let recipe = stone_recipe::from_slice(&recipe_bytes)?;
    let macros = Macros::load(&env)?;

    let paths = Paths::new(
        paths::Id::new(&recipe),
        &recipe_path,
        env.cache_dir,
        "/mason",
    )?;

    let rootfs = paths.rootfs().host;

    // Has rootfs been setup?
    if !rootfs.join("usr").exists() {
        return Err(Error::MissingRootFs);
    }

    // Generate a script so we can inject a .profile
    // to the container environment with all actions
    // and definitions
    //
    // The step doesn't matter, but we use `prepare`
    // since it uses hardcoded content that's always
    // available to create a script from
    let script = job::Step::Prepare
        .script(
            BuildTarget::Native(architecture::host()),
            None,
            &recipe,
            &paths,
            &macros,
            false,
        )
        .map_err(Error::BuildScript)?
        .expect("script always available for prepare step");
    let profile = &builder::build_profile(&script);

    let home = &paths.build().guest;

    container::exec(&paths, recipe.options.networking, || {
        fs::write(home.join(".profile"), profile)?;

        let mut child = process::Command::new("/bin/bash")
            .arg("--login")
            .env_clear()
            .env("HOME", home)
            .env("PATH", "/usr/bin:/usr/sbin")
            .env("TERM", "xterm-256color")
            .spawn()?;

        child.wait()?;

        Ok(())
    })?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("recipe file does not exist: {0:?}")]
    MissingRecipe(PathBuf),
    #[error("build root doesn't exist, make sure to run build first")]
    MissingRootFs,
    #[error("container")]
    Container(#[from] container::Error),
    #[error("macros")]
    Macros(#[from] macros::Error),
    #[error("stone recipe")]
    StoneRecipe(#[from] stone_recipe::Error),
    #[error("build script")]
    BuildScript(#[source] job::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
