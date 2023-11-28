// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::fs;
use std::io;
use std::path::PathBuf;

use boulder::{container, env, profile, Cache, Env, Runtime};
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
        recipe,
    } = command;
    let Global {
        moss_root,
        config_dir,
        cache_dir,
    } = global;

    if !output.exists() {
        return Err(Error::MissingOutput(output));
    }
    if !recipe.exists() {
        return Err(Error::MissingRecipe(recipe));
    }

    let recipe_bytes = fs::read(&recipe)?;
    let recipe = stone_recipe::from_slice(&recipe_bytes)?;

    let runtime = Runtime::new()?;
    let env = Env::new(config_dir, cache_dir, moss_root)?;

    let profiles = profile::Manager::new(&runtime, &env);
    let repos = profiles.repositories(&profile)?.clone();

    let cache = Cache::new(&recipe, &env.cache_dir, "/mason")?;
    let rootfs = cache.rootfs().host;

    env::recreate_dir(&rootfs)?;

    runtime.block_on(async {
        let mut moss_client = moss::Client::new("boulder", &env.moss_dir)
            .await?
            .explicit_repositories(repos)
            .await?
            .ephemeral(&rootfs)?;

        moss_client.install(BASE_PACKAGES, true).await?;

        Ok(()) as Result<(), Error>
    })?;

    // Drop async runtime
    drop(runtime);

    // TODO: Exec build scripts
    container::chroot(&cache).map_err(Error::Container)?;

    Ok(())
}

const BASE_PACKAGES: &[&str] = &[
    "bash",
    "boulder",
    "coreutils",
    "dash",
    "dbus",
    "dbus-broker",
    "file",
    "gawk",
    "git",
    "grep",
    "gzip",
    "inetutils",
    "iproute2",
    "less",
    "linux-kvm",
    "moss",
    "moss-container",
    "nano",
    "neofetch",
    "nss",
    "openssh",
    "procps",
    "python",
    "screen",
    "sed",
    "shadow",
    "sudo",
    "systemd",
    "unzip",
    "util-linux",
    "vim",
    "wget",
    "which",
];

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
    #[error("moss client")]
    MossClient(#[from] moss::client::Error),
    #[error("moss install")]
    MossInstall(#[from] moss::client::install::Error),
    #[error("stone recipe")]
    StoneRecipe(#[from] stone_recipe::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
