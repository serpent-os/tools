// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::{Path, PathBuf};

use clap::{arg, ArgMatches, Command};
use moss::{repository, Installation, Repository};
use thiserror::Error;
use tokio::runtime;
use url::Url;

pub fn command() -> Command {
    Command::new("repo")
        .about("Manage software repositories")
        .long_about("Manage the available software repositories visible to the installed system")
        .subcommand_required(true)
        .subcommand(
            Command::new("add")
                .arg(arg!(<NAME> "repo name").value_parser(clap::value_parser!(String)))
                .arg(arg!(<URI> "repo uri").value_parser(clap::value_parser!(Url))),
        )
}

/// Handle subcommands to `repo`
pub fn handle(args: &ArgMatches, root: &PathBuf) -> Result<(), Error> {
    match args.subcommand() {
        Some(("add", cmd_args)) => add_repo(cmd_args, root),
        _ => unreachable!(),
    }
}

/// moss repo add <NAME> <URI>
fn add_repo(args: &ArgMatches, root: &PathBuf) -> Result<(), Error> {
    let name = args.get_one::<String>("NAME").cloned().unwrap();
    let uri = args.get_one::<Url>("URI").cloned().unwrap();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(add(root, name, uri))
}

// Actual implementation of moss repo add, asynchronous
async fn add(root: &Path, name: String, uri: Url) -> Result<(), Error> {
    let installation = Installation::open(root);

    let mut manager = repository::Manager::new(installation).await?;

    manager
        .add_repository(
            repository::Id::new(name),
            Repository {
                description: "...".into(),
                uri,
                priority: 0,
            },
        )
        .await?;

    manager.refresh_all().await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("repo error: {0}")]
    RepositoryManager(#[from] repository::manager::Error),
}
