// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::{Path, PathBuf};

use clap::{arg, ArgMatches, Command};
use moss::{
    remote::{self, repository, Repository},
    Installation, Remote,
};
use thiserror::Error;
use tokio::runtime;
use url::Url;

pub fn command() -> Command {
    Command::new("remote")
        .about("...")
        .long_about("...")
        .arg(arg!(<NAME> "remote name").value_parser(clap::value_parser!(String)))
        .arg(arg!(<URL> "remote url").value_parser(clap::value_parser!(Url)))
}

pub fn handle(args: &ArgMatches, root: &PathBuf) -> Result<(), Error> {
    let name = args.get_one::<String>("NAME").cloned().unwrap();
    let url = args.get_one::<Url>("URL").cloned().unwrap();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(add(root, name, url))
}

async fn add(root: &Path, name: String, url: Url) -> Result<(), Error> {
    let installation = Installation::open(root);

    let mut remote = Remote::new(installation).await?;

    remote
        .add_repository(
            repository::Id::new(name),
            Repository {
                description: "...".into(),
                url,
                priority: 0,
            },
        )
        .await?;

    remote.refresh_all().await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("remote error: {0}")]
    Remote(#[from] remote::Error),
}
