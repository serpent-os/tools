// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;

use moss::repository;
use thiserror::Error;

use crate::{dependency, util, Env, Job};

pub async fn populate(
    env: &Env,
    job: &Job,
    repositories: repository::Map,
    ccache: bool,
) -> Result<(), Error> {
    let packages = dependency::calculate(&job.recipe, ccache);

    // Recreate root
    let rootfs = job.paths.rootfs().host;
    util::recreate_dir(&rootfs).await?;

    let mut moss_client = moss::Client::new("boulder", &env.moss_dir)
        .await?
        .explicit_repositories(repositories)
        .await?
        .ephemeral(&rootfs)?;

    moss_client.install(&packages, true).await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("moss client")]
    MossClient(#[from] moss::client::Error),
    #[error("moss install")]
    MossInstall(#[from] moss::client::install::Error),
}
