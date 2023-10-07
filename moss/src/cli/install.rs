// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{path::PathBuf, time::Duration};

use clap::{arg, ArgMatches, Command};
use futures::{future::join_all, stream, StreamExt, TryStreamExt};
use itertools::Itertools;
use moss::{
    client::{self, Client},
    package::{self, Flags},
    registry::transaction,
    Package,
};
use thiserror::Error;
use tui::{pretty::print_to_columns, MultiProgress, ProgressBar, ProgressStyle, Stylize};

use crate::cli::name_to_provider;

const CONCURRENT_TASKS: usize = 8;

pub fn command() -> Command {
    Command::new("install")
        .about("Install packages")
        .long_about("Install the requested software to the local system")
        .arg(arg!(<NAME> ... "packages to install").value_parser(clap::value_parser!(String)))
}

/// Resolve a package ID into either an error or a set of packages matching
/// TODO: Collapse to .first() for installation selection
async fn find_packages(id: &str, client: &Client) -> Result<Vec<Package>, Error> {
    let provider = name_to_provider(id);
    let result = client
        .registry
        .by_provider(&provider, Flags::AVAILABLE)
        .collect::<Vec<_>>()
        .await;
    if result.is_empty() {
        return Err(Error::NoCandidate(id.to_string()));
    }
    Ok(result)
}

/// Handle execution of `moss install`
pub async fn handle(args: &ArgMatches) -> Result<(), Error> {
    let root = args.get_one::<PathBuf>("root").unwrap().clone();

    let pkgs = args
        .get_many::<String>("NAME")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    // Grab a client for the target, enumerate packages
    let client = Client::new_for_root(root).await?;

    let queried = join_all(pkgs.iter().map(|p| find_packages(p, &client))).await;
    let (good_list, bad_list): (Vec<_>, Vec<_>) = queried.into_iter().partition(Result::is_ok);
    let bad: Vec<_> = bad_list.into_iter().map(Result::unwrap_err).collect();

    // TODO: Add error hookups
    if !bad.is_empty() {
        println!("Missing packages in lookup: {:?}", bad);
        return Err(Error::NotImplemented);
    }

    // The initial ids they want installed..
    let input = good_list
        .into_iter()
        .flat_map(Result::unwrap)
        .map(|r| (r.id.clone()))
        .collect::<Vec<_>>();

    // Try stuffing everything into the transaction now
    let mut tx = client.registry.transaction()?;
    tx.add(input).await?;

    // Resolve and map it. Remove any installed items. OK to unwrap here because they're resolved already
    let mut results = join_all(
        tx.finalize()
            .iter()
            .map(|p| async { client.registry.by_id(p).boxed().next().await.unwrap() }),
    )
    .await
    .into_iter()
    .filter(|p| !p.flags.contains(Flags::INSTALLED))
    .collect_vec();

    results.sort_by_key(|p| p.meta.name.to_string());
    results.dedup_by_key(|p| p.meta.name.to_string());

    println!("The following package(s) will be installed:");
    println!();
    print_to_columns(&results);
    println!();

    let multi_progress = MultiProgress::new();

    let total_progress = multi_progress.add(
        ProgressBar::new(results.len() as u64).with_style(
            ProgressStyle::with_template("\n[{bar:20.cyan/blue}] {pos}/{len}")
                .unwrap()
                .progress_chars("■≡=- "),
        ),
    );
    total_progress.tick();

    // Download and unpack each package
    stream::iter(results.into_iter().map(|package| async {
        // Setup the progress bar and set as downloading
        let progress = multi_progress.insert_before(
            &total_progress,
            ProgressBar::new(1000)
                .with_message(format!(
                    "{} {}",
                    "Downloading".blue(),
                    package.meta.name.to_string().bold(),
                ))
                .with_style(
                    ProgressStyle::with_template("{spinner} [{percent:>3}%] {msg}").unwrap(),
                ),
        );
        progress.enable_steady_tick(Duration::from_millis(100));

        // Download and update progress
        let download = package::fetch(&package.meta, &client.installation, |pct| {
            progress.set_position((pct * 1000.0) as u64);
        })
        .await?;

        // Set progress to unpacking
        progress.set_message(format!(
            "{} {}",
            "Unpacking".yellow(),
            package.meta.name.to_string().bold(),
        ));
        progress.set_position(0);

        // Unpack and update progress
        download
            .unpack({
                let pb = progress.clone();

                move |progress| {
                    pb.set_position((progress * 1000.0) as u64);
                }
            })
            .await?;

        // Write installed line
        multi_progress.println(format!(
            "{} {}",
            "Installed".green(),
            package.meta.name.to_string().bold(),
        ))?;

        // Remove this progress bar
        progress.finish();
        multi_progress.remove(&progress);

        // Inc total progress by 1
        total_progress.inc(1);

        // Get smarter borrow checker
        drop(package);

        Ok(()) as Result<(), Error>
    }))
    .buffer_unordered(CONCURRENT_TASKS)
    .try_collect()
    .await?;

    // Remove progress
    multi_progress.clear()?;

    Err(Error::NotImplemented)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("client error")]
    Client(#[from] client::Error),

    #[error("no such candidate: {0}")]
    NoCandidate(String),

    #[error("not yet implemented")]
    NotImplemented,

    #[error("transaction error: {0}")]
    Transaction(#[from] transaction::Error),

    #[error("package fetch error: {0}")]
    Package(#[from] package::fetch::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
