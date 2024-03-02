// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{path::Path, process};

use clap::{arg, Arg, ArgAction, ArgMatches, Command};
use itertools::Itertools;
use moss::{
    repository::{self, Priority},
    runtime, Installation, Repository,
};
use thiserror::Error;
use url::Url;

/// Control flow for the subcommands
enum Action<'a> {
    // Root
    List(&'a Path),
    // Root, Id, Url, Comment
    Add(&'a Path, String, Url, String, Priority),
    // Root, Id
    Remove(&'a Path, String),
    // Root, Id
    Update(&'a Path, Option<String>),
}

/// Return a command for handling `repo` subcommands
pub fn command() -> Command {
    Command::new("repo")
        .about("Manage software repositories")
        .long_about("Manage the available software repositories visible to the installed system")
        .subcommand_required(true)
        .subcommand(
            Command::new("add")
                .visible_alias("ar")
                .arg(arg!(<NAME> "repo name").value_parser(clap::value_parser!(String)))
                .arg(arg!(<URI> "repo uri").value_parser(clap::value_parser!(Url)))
                .arg(
                    Arg::new("comment")
                        .short('c')
                        .default_value("...")
                        .action(ArgAction::Set)
                        .help("Set the comment for the repository")
                        .value_parser(clap::value_parser!(String)),
                )
                .arg(
                    Arg::new("priority")
                        .short('p')
                        .help("Repository priority")
                        .action(ArgAction::Set)
                        .default_value("0")
                        .value_parser(clap::value_parser!(u64)),
                ),
        )
        .subcommand(
            Command::new("list")
                .visible_alias("lr")
                .about("List system software repositories")
                .long_about("List all of the system repositories and their status"),
        )
        .subcommand(
            Command::new("remove")
                .visible_alias("rr")
                .about("Remove a repository for the system")
                .arg(arg!(<NAME> "repo name").value_parser(clap::value_parser!(String))),
        )
        .subcommand(
            Command::new("update")
                .visible_alias("ur")
                .about("Update the system repositories")
                .long_about("If no repository is named, update them all")
                .arg(arg!([NAME] "repo name").value_parser(clap::value_parser!(String))),
        )
}

/// Handle subcommands to `repo`
pub fn handle(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    let config = config::Manager::system(root, "moss");

    let handler = match args.subcommand() {
        Some(("add", cmd_args)) => Action::Add(
            root,
            cmd_args.get_one::<String>("NAME").cloned().unwrap(),
            cmd_args.get_one::<Url>("URI").cloned().unwrap(),
            cmd_args.get_one::<String>("comment").cloned().unwrap(),
            Priority::new(*cmd_args.get_one::<u64>("priority").unwrap()),
        ),
        Some(("list", _)) => Action::List(root),
        Some(("remove", cmd_args)) => Action::Remove(root, cmd_args.get_one::<String>("NAME").cloned().unwrap()),
        Some(("update", cmd_args)) => Action::Update(root, cmd_args.get_one::<String>("NAME").cloned()),
        _ => unreachable!(),
    };

    // dispatch to runtime handler function
    match handler {
        Action::List(root) => list(root, config),
        Action::Add(root, name, uri, comment, priority) => add(root, config, name, uri, comment, priority),
        Action::Remove(root, name) => remove(root, config, name),
        Action::Update(root, name) => update(root, config, name),
    }
}

// Actual implementation of moss repo add
fn add(
    root: &Path,
    config: config::Manager,
    name: String,
    uri: Url,
    comment: String,
    priority: Priority,
) -> Result<(), Error> {
    let installation = Installation::open(root);

    let mut manager = repository::Manager::system(config, installation)?;

    let id = repository::Id::new(name);

    manager.add_repository(
        id.clone(),
        Repository {
            description: comment,
            uri,
            priority,
        },
    )?;

    runtime::block_on(manager.refresh(&id))?;

    println!("{id} added");

    Ok(())
}

/// List the repositories and pretty print them
fn list(root: &Path, config: config::Manager) -> Result<(), Error> {
    let installation = Installation::open(root);
    let manager = repository::Manager::system(config, installation)?;

    let configured_repos = manager.list();
    if configured_repos.len() == 0 {
        println!("No repositories have been configured yet");
        return Ok(());
    }

    for (id, repo) in configured_repos.sorted_by(|(_, a), (_, b)| a.priority.cmp(&b.priority).reverse()) {
        println!(" - {} = {} [{}]", id, repo.uri, repo.priority);
    }

    Ok(())
}

/// Update specific repos or all
fn update(root: &Path, config: config::Manager, which: Option<String>) -> Result<(), Error> {
    let installation = Installation::open(root);
    let mut manager = repository::Manager::system(config, installation)?;

    runtime::block_on(async {
        match which {
            Some(repo) => manager.refresh(&repository::Id::new(repo)).await,
            None => manager.refresh_all().await,
        }
    })?;

    Ok(())
}

/// Remove repo
fn remove(root: &Path, config: config::Manager, repo: String) -> Result<(), Error> {
    let id = repository::Id::new(repo);

    let installation = Installation::open(root);
    let mut manager = repository::Manager::system(config, installation)?;

    match manager.remove(id.clone())? {
        repository::manager::Removal::NotFound => {
            println!("{id} not found");
            process::exit(1);
        }
        repository::manager::Removal::ConfigDeleted(false) => {
            println!(
                "{id} configuration must be manually deleted since it doesn't exist in it's own configuration file"
            );
            process::exit(1);
        }
        repository::manager::Removal::ConfigDeleted(true) => {
            println!("{id} removed");
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("repo manager")]
    RepositoryManager(#[from] repository::manager::Error),
}
