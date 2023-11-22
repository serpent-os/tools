// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::path::PathBuf;

use clap::{Args, Parser};
use thiserror::Error;

mod build;
mod chroot;

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
    #[arg(long, global = true, default_value = "/")]
    pub moss_root: PathBuf,
    #[arg(long, global = true)]
    pub config_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub cache_dir: Option<PathBuf>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    Build(build::Command),
    Chroot(chroot::Command),
}

pub async fn process() -> Result<(), Error> {
    let Command { global, subcommand } = Command::parse();

    match subcommand {
        Subcommand::Build(command) => build::handle(command, global).await?,
        Subcommand::Chroot(command) => chroot::handle(command, global).await?,
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("build")]
    Build(#[from] build::Error),
    #[error("chroot")]
    Chroot(#[from] chroot::Error),
}
