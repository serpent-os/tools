// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{path::PathBuf, str::FromStr};

use clap::{Arg, ArgAction, Command};
use moss::Provider;
use thiserror::Error;

mod pretty;

mod extract;
mod info;
mod inspect;
mod install;
mod list;
mod remove;
mod repo;
mod version;

/// Convert the name to a lookup provider
pub(crate) fn name_to_provider(name: &str) -> Provider {
    if name.contains('(') {
        Provider::from_str(name).unwrap()
    } else {
        Provider {
            kind: moss::dependency::Kind::PackageName,
            name: name.to_owned(),
        }
    }
}

/// Generate the CLI command structure
fn command() -> Command {
    Command::new("moss")
        .about("Next generation package manager")
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("root")
                .short('D')
                .long("directory")
                .global(true)
                .help("Root directory")
                .action(ArgAction::Set)
                .default_value("/")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes-all")
                .global(true)
                .help("Assume yes for all questions")
                .action(ArgAction::SetTrue),
        )
        .arg_required_else_help(true)
        .subcommand(extract::command())
        .subcommand(info::command())
        .subcommand(inspect::command())
        .subcommand(install::command())
        .subcommand(list::command())
        .subcommand(remove::command())
        .subcommand(version::command())
        .subcommand(repo::command())
}

/// Process all CLI arguments
pub async fn process() -> Result<(), Error> {
    let matches = command().get_matches();
    if matches.get_flag("version") {
        version::print();
        return Ok(());
    }

    let root = matches.get_one::<PathBuf>("root").unwrap();

    match command().get_matches().subcommand() {
        Some(("extract", args)) => extract::handle(args).await.map_err(Error::Extract),
        Some(("info", args)) => info::handle(args).await.map_err(Error::Info),
        Some(("inspect", args)) => inspect::handle(args).await.map_err(Error::Inspect),
        Some(("install", args)) => install::handle(args).await.map_err(Error::Install),
        Some(("version", _)) => {
            version::print();
            Ok(())
        }
        Some(("list", a)) => list::handle(a).await.map_err(Error::List),
        Some(("repo", a)) => repo::handle(a, root).await.map_err(Error::Repo),
        _ => unreachable!(),
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("error handling info: {0}")]
    Info(#[from] info::Error),

    #[error("error handling install: {0}")]
    Install(#[from] install::Error),

    #[error("error handling list: {0}")]
    List(#[from] list::Error),

    #[error("error handling inspect: {0}")]
    Inspect(#[from] inspect::Error),

    #[error("error in extraction: {0}")]
    Extract(#[from] extract::Error),

    #[error("error handling repo: {0}")]
    Repo(#[from] repo::Error),
}
