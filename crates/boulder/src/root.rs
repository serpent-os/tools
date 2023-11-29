// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;

use moss::repository;
use stone_recipe::Recipe;
use thiserror::Error;

use crate::{dependency, env, Cache, Env};

pub async fn populate(
    env: &Env,
    cache: &Cache,
    repositories: repository::Map,
    recipe: &Recipe,
    ccache: bool,
) -> Result<(), Error> {
    let packages = dependency::calculate(recipe, ccache);

    // Recreate root
    let rootfs = cache.rootfs().host;
    env::recreate_dir(&rootfs)?;

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
