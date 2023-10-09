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
use stone::read::Payload;
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
            ProgressStyle::with_template("\n|{bar:20.cyan/blue}| {pos}/{len}")
                .unwrap()
                .progress_chars("■≡=- "),
        ),
    );
    total_progress.tick();

    // Download and unpack each package
    stream::iter(results.into_iter().map(|package| async {
        // Setup the progress bar and set as downloading
        let progress_bar = multi_progress.insert_before(
            &total_progress,
            ProgressBar::new(package.meta.download_size.unwrap_or_default())
                .with_message(format!(
                    "{} {}",
                    "Downloading".blue(),
                    package.meta.name.to_string().bold(),
                ))
                .with_style(
                    ProgressStyle::with_template("{spinner} |{percent:>3}%| {msg}")
                        .unwrap()
                        .tick_chars("--=≡■≡=--"),
                ),
        );
        progress_bar.enable_steady_tick(Duration::from_millis(150));

        // Download and update progress
        let download = package::fetch(&package.meta, &client.installation, |progress| {
            progress_bar.inc(progress.delta);
        })
        .await?;

        let package_name = package.meta.name.to_string();

        // Set progress to unpacking
        progress_bar.set_message(format!(
            "{} {}",
            "Unpacking".yellow(),
            package_name.clone().bold(),
        ));
        progress_bar.set_length(1000);
        progress_bar.set_position(0);

        // Unpack and update progress
        let unpacked = download
            .unpack({
                let progress_bar = progress_bar.clone();

                move |progress| {
                    progress_bar.set_position((progress.pct() * 1000.0) as u64);
                }
            })
            .await?;

        // Merge into the DB!
        progress_bar.set_message(format!(
            "{} {}",
            "Accounting".white(),
            package_name.clone().bold()
        ));
        progress_bar.set_length(2);
        progress_bar.set_position(0);

        // Merge layoutdb
        for chunk in unpacked
            .payloads
            .iter()
            .find_map(Payload::layout)
            .ok_or(Error::CorruptedPackage)?
            .chunks(1000)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|i| (package.id.clone(), i.clone()))
                    .collect_vec()
            })
        {
            client
                .layout_db
                .batch_add(chunk)
                .await
                .map_err(Error::LayoutDB)?;
        }

        progress_bar.set_position(1);

        // Consume the package
        client.install_db.add(package.id, package.meta).await?;
        progress_bar.set_position(2);

        // Write installed line
        multi_progress.println(format!(
            "{} {}",
            "Installed".green(),
            package_name.clone().bold(),
        ))?;

        // Remove this progress bar
        progress_bar.finish();
        multi_progress.remove(&progress_bar);

        // Inc total progress by 1
        total_progress.inc(1);

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

    #[error("corrupted package")]
    CorruptedPackage,

    #[error("no such candidate: {0}")]
    NoCandidate(String),

    #[error("not yet implemented")]
    NotImplemented,

    #[error("transaction error: {0}")]
    Transaction(#[from] transaction::Error),

    #[error("package fetch error: {0}")]
    Package(#[from] package::fetch::Error),

    #[error("installdb error: {0}")]
    InstallDB(#[from] moss::db::meta::Error),

    #[error("layoutdb error: {0}")]
    LayoutDB(#[from] moss::db::layout::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
