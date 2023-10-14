// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::{ArgMatches, Command};
use futures::StreamExt;
use itertools::Itertools;
use thiserror::Error;

use moss::{
    client::{self, Client},
    package::Flags,
};
use tui::Stylize;

pub fn command() -> Command {
    Command::new("list")
        .about("List packages")
        .long_about("List packages according to a filter")
        .subcommand_required(true)
        .subcommand(
            Command::new("installed")
                .about("List all installed packages")
                .visible_alias("li"),
        )
        .subcommand(
            Command::new("available")
                .about("List all available packages")
                .visible_alias("la"),
        )
}

/// Handle listing by filter
pub async fn handle(args: &ArgMatches) -> Result<(), Error> {
    let root = args.get_one::<PathBuf>("root").unwrap().clone();

    let filter_flags = match args.subcommand() {
        Some(("available", _)) => Flags::AVAILABLE,
        Some(("installed", _)) => Flags::INSTALLED,
        _ => unreachable!(),
    };

    // Grab a client for the target, enumerate packages
    let client = Client::new_for_root(root).await?;
    let pkgs = client.registry.list(filter_flags).collect::<Vec<_>>().await;

    if pkgs.is_empty() {
        return Err(Error::NoneFound);
    }

    // map to renderable state
    let mut set = pkgs
        .into_iter()
        .map(|p| State {
            name: p.meta.name.to_string(),
            version: p.meta.version_identifier,
            release: p.meta.source_release.to_string(),
            summary: p.meta.summary,
        })
        .collect_vec();
    // sort alpha
    set.sort();

    // Grab maximum field
    let max_element = set
        .iter()
        .max_by_key(|p| p.name.len() + p.release.len() + p.version.len())
        .unwrap();
    let max_length = max_element.name.len() + max_element.version.len() + max_element.version.len();

    // render
    for st in set {
        let width = (max_length - (st.name.len() + st.release.len() + st.version.len())) + 2;
        println!(
            "{} {:width$} {}-{} - {}",
            st.name.bold(),
            " ",
            st.version.magenta(),
            st.release.dim(),
            st.summary,
            width = width
        );
    }

    Ok(())
}

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord)]
struct State {
    name: String,
    version: String,
    summary: String,
    release: String,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client error: {0}")]
    Client(#[from] client::Error),

    #[error("no packages found")]
    NoneFound,
}
