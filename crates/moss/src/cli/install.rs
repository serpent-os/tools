// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::{Path, PathBuf};

use clap::{arg, value_parser, ArgMatches, Command};
use moss::{client::Client, environment};

pub use moss::client::install::Error;

pub fn command() -> Command {
    Command::new("install")
        .about("Install packages")
        .long_about("Install the requested software to the local system")
        .arg(arg!(<NAME> ... "packages to install").value_parser(value_parser!(String)))
        .arg(
            arg!(--to <blit_target> "Blit this install to the provided directory instead of the root")
                .long_help(
                    "Blit this install to the provided directory instead of the root. \n\
                     \n\
                     This operation won't be captured as a new state",
                )
                .value_parser(value_parser!(PathBuf)),
        )
}

/// Handle execution of `moss install`
pub async fn handle(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    let pkgs = args
        .get_many::<String>("NAME")
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let yes = *args.get_one::<bool>("yes").unwrap();

    // Grab a client for the root
    let mut client = Client::new(environment::NAME, root).await?;

    // Make ephemeral if a blit target was provided
    if let Some(blit_target) = args.get_one::<PathBuf>("to").cloned() {
        client = client.ephemeral(blit_target)?;
    }

    client.install(&pkgs, yes).await
}
