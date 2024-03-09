// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{env, path::PathBuf};

use clap::{Arg, ArgAction, Command};
use color_eyre::{eyre::Context, Result, Section};
use moss::{runtime, Installation};

mod extract;
mod index;
mod info;
mod inspect;
mod install;
mod list;
mod remove;
mod repo;
mod state;
mod sync;
mod version;

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
            Arg::new("cache")
                .long("cache")
                .global(true)
                .help("Cache directory")
                .action(ArgAction::Set)
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
        .subcommand(index::command())
        .subcommand(info::command())
        .subcommand(inspect::command())
        .subcommand(install::command())
        .subcommand(list::command())
        .subcommand(remove::command())
        .subcommand(repo::command())
        .subcommand(state::command())
        .subcommand(sync::command())
        .subcommand(version::command())
}

/// Process all CLI arguments
pub fn process() -> Result<()> {
    let args = replace_aliases(env::args());
    let matches = command().get_matches_from(args);

    if matches.get_flag("version") {
        version::print();
        return Ok(());
    }

    let root = matches.get_one::<PathBuf>("root").unwrap();
    let cache = matches.get_one::<PathBuf>("cache");

    // Make async runtime available to all of moss
    let _guard = runtime::init();

    let mut installation = Installation::open(root)
        .with_context(|| format!("open root directory {root:?}"))
        .suggestion("ensure root exists and is a directory")?;

    if let Some(dir) = cache {
        installation = installation
            .with_cache_dir(dir)
            .with_context(|| format!("use cache directory {dir:?}"))
            .suggestion("ensure cache exists and is a directory")?;
    }

    match matches.subcommand() {
        Some(("extract", args)) => extract::handle(args)?,
        Some(("index", args)) => index::handle(args)?,
        Some(("info", args)) => info::handle(args, installation)?,
        Some(("inspect", args)) => inspect::handle(args)?,
        Some(("install", args)) => install::handle(args, installation)?,
        Some(("list", args)) => list::handle(args, installation)?,
        Some(("remove", args)) => remove::handle(args, installation)?,
        Some(("repo", args)) => repo::handle(args, installation)?,
        Some(("state", args)) => state::handle(args, installation)?,
        Some(("sync", args)) => sync::handle(args, installation)?,
        Some(("version", _)) => {
            version::print();
        }
        None => {
            command().print_help().unwrap();
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn replace_aliases(args: env::Args) -> Vec<String> {
    const ALIASES: &[(&str, &[&str])] = &[
        ("li", &["list", "installed"]),
        ("la", &["list", "available"]),
        ("ls", &["list", "sync"]),
        ("lu", &["list", "sync"]),
        ("ar", &["repo", "add"]),
        ("lr", &["repo", "list"]),
        ("rr", &["repo", "remove"]),
        ("ur", &["repo", "update"]),
        ("ix", &["index"]),
        ("it", &["install"]),
        ("rm", &["remove"]),
        ("up", &["sync"]),
    ];

    let mut args = args.collect::<Vec<_>>();

    for (alias, replacements) in ALIASES {
        let Some(pos) = args.iter().position(|a| a == *alias) else {
            continue;
        };

        args.splice(pos..pos + 1, replacements.iter().map(|arg| arg.to_string()));

        break;
    }

    args
}
