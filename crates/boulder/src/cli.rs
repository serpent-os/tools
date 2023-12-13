// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::path::PathBuf;

use boulder::{env, Env};
use clap::{Args, Parser};
use thiserror::Error;

mod build;
mod chroot;
mod profile;

#[derive(Debug, Parser)]
#[command()]
pub struct Command {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub subcommand: Subcommand,
}

#[derive(Debug, Args)]
pub struct Global {
    #[arg(long, global = true)]
    pub cache_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub config_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub moss_root: Option<PathBuf>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    Build(build::Command),
    Chroot(chroot::Command),
    Profile(profile::Command),
}

pub fn process() -> Result<(), Error> {
    let Command { global, subcommand } = Command::parse();

    let env = Env::new(
        global.cache_dir,
        global.config_dir,
        global.data_dir,
        global.moss_root,
    )?;

    match subcommand {
        Subcommand::Build(command) => build::handle(command, env)?,
        Subcommand::Chroot(command) => chroot::handle(command, env)?,
        Subcommand::Profile(command) => profile::handle(command, env)?,
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("build")]
    Build(#[from] build::Error),
    #[error("chroot")]
    Chroot(#[from] chroot::Error),
    #[error("profile")]
    Profile(#[from] profile::Error),
    #[error("env")]
    Env(#[from] env::Error),
}
