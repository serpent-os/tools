// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use clap::{arg, ArgAction, ArgMatches, Command};
use futures::{stream, StreamExt, TryFutureExt, TryStreamExt};
use moss::{
    client::{self, prune, Client},
    state,
};
use thiserror::Error;
use tui::pretty::print_to_columns;

pub fn command() -> Command {
    Command::new("state")
        .about("Manage state")
        .long_about("Manage state ...")
        .subcommand_required(true)
        .subcommand(Command::new("list").about("List all states"))
        .subcommand(
            Command::new("prune").about("Prune old states").arg(
                arg!(-k --keep "Keep this many states")
                    .action(ArgAction::Set)
                    .default_value("10")
                    .value_parser(clap::value_parser!(u64).range(1..)),
            ),
        )
}

pub async fn handle(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    match args.subcommand() {
        Some(("list", _)) => list(root).await,
        Some(("prune", args)) => prune(args, root).await,
        _ => unreachable!(),
    }
}

pub async fn list(root: &Path) -> Result<(), Error> {
    let client = Client::new_for_root(root).await?;

    let state_ids = client.state_db.list_ids().await?;

    let states = stream::iter(state_ids.iter().map(|(id, _)| id))
        .then(|id| client.state_db.get(id).map_err(Error::StateDB))
        .try_collect::<Vec<_>>()
        .await?;

    print_to_columns(&states.iter().map(state::ColumnDisplay).collect::<Vec<_>>());

    Ok(())
}

pub async fn prune(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    let keep = *args.get_one::<u64>("keep").unwrap();

    let client = Client::new_for_root(root).await?;
    client.prune(prune::Strategy::KeepRecent(keep)).await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client error: {0}")]
    Client(#[from] client::Error),

    #[error("statedb error: {0}")]
    StateDB(#[from] moss::db::state::Error),
}
