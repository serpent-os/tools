// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{env, path::PathBuf};

use clap::{Arg, ArgAction, Command};
use clap_mangen::Man;
use moss::{installation, runtime, Installation};
use thiserror::Error;

mod extract;
mod index;
mod info;
mod inspect;
mod install;
mod list;
mod remove;
mod repo;
mod search;
mod state;
mod sync;
mod version;

/// Generate the CLI command structure
fn command() -> Command {
    Command::new("moss")
        .about("Next generation package manager")
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .global(true)
                .help("Prints additional information about what moss is doing")
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
        .arg(
            Arg::new("generate-manpages")
                .long("generate-manpages")
                .help("Generate manpages")
                .action(ArgAction::Set)
                .value_name("DIR")
                .hide(true),
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
        .subcommand(search::command())
        .subcommand(state::command())
        .subcommand(sync::command())
        .subcommand(version::command())
}

/// Generate manpages for all commands recursively
fn generate_manpages(cmd: &Command, dir: &std::path::Path, prefix: Option<&str>) -> std::io::Result<()> {
    let name = cmd.get_name();
    let man = Man::new(cmd.to_owned());
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    let filename = if let Some(prefix) = prefix {
        format!("{}-{}.1", prefix, name)
    } else {
        format!("{}.1", name)
    };

    std::fs::write(dir.join(filename), buffer)?;

    for subcmd in cmd.get_subcommands() {
        let new_prefix = if let Some(p) = prefix {
            format!("{}-{}", p, name)
        } else {
            name.to_string()
        };
        generate_manpages(subcmd, dir, Some(&new_prefix))?;
    }
    Ok(())
}

/// Process all CLI arguments
pub fn process() -> Result<(), Error> {
    let args = replace_aliases(env::args());
    let matches = command().get_matches_from(args);

    if let Some(dir) = matches.get_one::<String>("generate-manpages") {
        let dir = std::path::Path::new(dir);
        std::fs::create_dir_all(dir)?;
        generate_manpages(&command(), dir, None)?;
        return Ok(());
    }

    // Print the version, but not if the user is using the version subcommand
    if matches.get_flag("verbose") {
        if let Some(command) = matches.subcommand_name() {
            if command != "version" {
                version::print();
            }
        }
    }

    let root = matches.get_one::<PathBuf>("root").unwrap();
    let cache = matches.get_one::<PathBuf>("cache");

    // Make async runtime available to all of moss
    let _guard = runtime::init();

    let installation = Installation::open(root, cache.cloned())?;

    match matches.subcommand() {
        Some(("extract", args)) => extract::handle(args).map_err(Error::Extract),
        Some(("index", args)) => index::handle(args).map_err(Error::Index),
        Some(("info", args)) => info::handle(args, installation).map_err(Error::Info),
        Some(("inspect", args)) => inspect::handle(args).map_err(Error::Inspect),
        Some(("install", args)) => install::handle(args, installation).map_err(Error::Install),
        Some(("list", args)) => list::handle(args, installation).map_err(Error::List),
        Some(("remove", args)) => remove::handle(args, installation).map_err(Error::Remove),
        Some(("repo", args)) => repo::handle(args, installation).map_err(Error::Repo),
        Some(("search", args)) => search::handle(args, installation).map_err(Error::Search),
        Some(("state", args)) => state::handle(args, installation).map_err(Error::State),
        Some(("sync", args)) => sync::handle(args, installation).map_err(Error::Sync),
        Some(("version", args)) => {
            version::handle(args);
            Ok(())
        }
        None => {
            command().print_help().unwrap();
            Ok(())
        }
        _ => unreachable!(),
    }
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
        ("er", &["repo", "enable"]),
        ("dr", &["repo", "disable"]),
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("index")]
    Index(#[from] index::Error),

    #[error("info")]
    Info(#[from] info::Error),

    #[error("install")]
    Install(#[from] install::Error),

    #[error("list")]
    List(#[from] list::Error),

    #[error("inspect")]
    Inspect(#[from] inspect::Error),

    #[error("extract")]
    Extract(#[from] extract::Error),

    #[error("remove")]
    Remove(#[from] remove::Error),

    #[error("repo")]
    Repo(#[from] repo::Error),

    #[error("search")]
    Search(#[from] search::Error),

    #[error("state")]
    State(#[from] state::Error),

    #[error("sync")]
    Sync(#[from] sync::Error),

    #[error("installation")]
    Installation(#[from] installation::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
