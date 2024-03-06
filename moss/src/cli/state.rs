// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{arg, ArgAction, ArgMatches, Command};
use moss::{
    client::{self, prune, Client},
    environment, state, Installation,
};
use thiserror::Error;
use tui::Styled;

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

pub fn handle(args: &ArgMatches, installation: Installation) -> Result<(), Error> {
    match args.subcommand() {
        Some(("list", _)) => list(installation),
        Some(("prune", args)) => prune(args, installation),
        _ => unreachable!(),
    }
}

/// List all known states, newest first
pub fn list(installation: Installation) -> Result<(), Error> {
    let client = Client::new(environment::NAME, installation)?;

    let state_ids = client.state_db.list_ids()?;

    let mut states = state_ids
        .iter()
        .map(|(id, _)| client.state_db.get(id).map_err(Error::StateDB))
        .collect::<Result<Vec<_>, _>>()?;

    states.reverse();
    states.into_iter().for_each(print_state);
    Ok(())
}

pub fn prune(args: &ArgMatches, installation: Installation) -> Result<(), Error> {
    let keep = *args.get_one::<u64>("keep").unwrap();

    let client = Client::new(environment::NAME, installation)?;
    client.prune(prune::Strategy::KeepRecent(keep))?;

    Ok(())
}

/// Emit a state description for the TUI
fn print_state(state: state::State) {
    println!(
        "State #{} - {}",
        state.id.to_string().bold(),
        state.summary.unwrap_or(String::from("system transaction")),
    );
    println!("{} {}", "Created:".bold(), state.created);
    println!(
        "{} {}",
        "Description:".bold(),
        state.description.unwrap_or(String::from("no description"))
    );
    // TODO: List packages?
    // TODO: Start with normal list, compute diff, reverse to print ?
    println!("{} {}", "Packages:".bold(), state.selections.len());
    println!();
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client")]
    Client(#[from] client::Error),

    #[error("state db")]
    StateDB(#[from] moss::db::state::Error),
}
