// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use clap::{arg, Arg, ArgAction, ArgMatches, Command};
use itertools::Itertools;
use moss::{repository, Installation, Repository};
use thiserror::Error;
use url::Url;

/// Control flow for the subcommands
enum Action<'a> {
    // Root
    List(&'a Path),
    // Root, Id, Url, Comment
    Add(&'a Path, String, Url, String),
    // Root, Id
    Remove(&'a Path, String),
}

/// Return a command for handling `repo` subcommands
pub fn command() -> Command {
    Command::new("repo")
        .about("Manage software repositories")
        .long_about("Manage the available software repositories visible to the installed system")
        .subcommand_required(true)
        .subcommand(
            Command::new("add")
                .arg(arg!(<NAME> "repo name").value_parser(clap::value_parser!(String)))
                .arg(arg!(<URI> "repo uri").value_parser(clap::value_parser!(Url)))
                .arg(
                    Arg::new("comment")
                        .short('c')
                        .default_value("...")
                        .action(ArgAction::Set)
                        .help("Set the comment for the repository")
                        .value_parser(clap::value_parser!(String)),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List system software repositories")
                .long_about("List all of the system repositories and their status"),
        )
        .subcommand(
            Command::new("remove")
                .about("Remove a repository for the system")
                .arg(arg!(<NAME> "repo name").value_parser(clap::value_parser!(String))),
        )
}

/// Handle subcommands to `repo`
pub async fn handle(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    let handler = match args.subcommand() {
        Some(("add", cmd_args)) => Action::Add(
            root,
            cmd_args.get_one::<String>("NAME").cloned().unwrap(),
            cmd_args.get_one::<Url>("URI").cloned().unwrap(),
            cmd_args.get_one::<String>("comment").cloned().unwrap(),
        ),
        Some(("list", _)) => Action::List(root),
        Some(("remove", cmd_args)) => {
            Action::Remove(root, cmd_args.get_one::<String>("NAME").cloned().unwrap())
        }
        _ => unreachable!(),
    };

    // dispatch to runtime handler function
    match handler {
        Action::List(root) => list(root).await,
        Action::Add(root, name, uri, comment) => add(root, name, uri, comment).await,
        Action::Remove(_, _) => unimplemented!(),
    }
}

// Actual implementation of moss repo add, asynchronous
async fn add(root: &Path, name: String, uri: Url, comment: String) -> Result<(), Error> {
    let installation = Installation::open(root);

    let mut manager = repository::Manager::new(installation).await?;

    manager
        .add_repository(
            repository::Id::new(name),
            Repository {
                description: comment,
                uri,
                priority: repository::Priority::new(0),
            },
        )
        .await?;

    manager.refresh_all().await?;

    Ok(())
}

/// List the repositories and pretty print them
async fn list(root: &Path) -> Result<(), Error> {
    let installation = Installation::open(root);
    let manager = repository::Manager::new(installation).await?;

    let configured_repos = manager.list();
    if configured_repos.len() == 0 {
        println!("No repositories have been configured yet");
        return Ok(());
    }

    for (id, repo) in
        configured_repos.sorted_by(|(_, a), (_, b)| a.priority.cmp(&b.priority).reverse())
    {
        println!(" - {} = {} [{}]", id, repo.uri, repo.priority);
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("repo manager")]
    RepositoryManager(#[from] repository::manager::Error),
}
