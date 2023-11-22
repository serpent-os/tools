// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::process;

use boulder::{client, Client};
use clap::Parser;
use thiserror::Error;
use tokio::task;

use super::Global;

#[derive(Debug, Parser)]
#[command(about = "chroot into the build environment")]
pub struct Command {}

pub async fn handle(_command: Command, global: Global) -> Result<(), Error> {
    let Global {
        config_dir,
        cache_dir,
        ..
    } = global;

    let client = Client::new(config_dir, cache_dir).await?;

    let ephemeral_root = client.cache.join("test-root");

    task::spawn_blocking(move || {
        container::run(ephemeral_root, move || {
            let mut child = process::Command::new("/bin/bash")
                .arg("--login")
                .env_clear()
                .env("HOME", "/root")
                .env("PATH", "/usr/bin:/usr/sbin")
                .env("TERM", "xterm-256color")
                .spawn()?;

            child.wait()?;

            Ok(())
        })
    })
    .await
    .expect("join handle")
    .map_err(Error::Container)?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("container")]
    Container(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("client")]
    Client(#[from] client::Error),
}
