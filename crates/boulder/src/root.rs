// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;

use moss::repository;
use thiserror::Error;

use crate::{container, dependency, util, Builder};

pub async fn populate(builder: &Builder, repositories: repository::Map) -> Result<(), Error> {
    let packages = dependency::calculate(builder);

    let rootfs = builder.paths.rootfs().host;

    // Recreate root
    util::recreate_dir(&rootfs).await?;

    let mut moss_client = moss::Client::with_explicit_repositories("boulder", &builder.env.moss_dir, repositories)
        .await?
        .ephemeral(&rootfs)?;

    moss_client.install(&packages, true).await?;

    Ok(())
}

pub fn clean(builder: &Builder) -> Result<(), Error> {
    // Dont't need to clean if it doesn't exist
    if !builder.paths.build().host.exists() {
        return Ok(());
    }

    // We recreate inside the container so we don't
    // get permissions error if this is a rootless build
    // and there's subuid mappings into the user namespace
    container::exec(&builder.paths, false, || {
        // Recreate `install` dir
        util::sync::recreate_dir(&builder.paths.install().guest)?;

        for target in &builder.targets {
            for job in &target.jobs {
                // Recerate build dir
                util::sync::recreate_dir(&job.build_dir)?;
            }
        }

        Ok(())
    })?;

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
    #[error("container")]
    Container(#[from] container::Error),
}
