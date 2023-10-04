// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::BTreeMap, path::PathBuf, time::Instant};

use clap::{arg, ArgMatches, Command};
use futures::{future::join_all, sink, stream, StreamExt, TryStreamExt};
use itertools::Itertools;
use moss::{
    client::{self, Client},
    package::{self, Flags},
    registry::transaction,
    Package,
};
use thiserror::Error;
use tui::{
    pretty::print_to_columns,
    widget::{progress, Line, Paragraph},
    Constraint, Direction, Layout, TuiStylize,
};

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

    tui::run(Program::new(results.len()), |handle| async move {
        // Download and unpack each package
        stream::iter(results.into_iter().map(|package| async {
            handle.update(Message::Downloading(package.meta.name.to_string(), 0.0));
            let download = package::fetch(
                &package.meta,
                &client.installation,
                Box::pin(sink::unfold((), |(), progress| {
                    let handle = handle.clone();
                    let name = package.meta.name.to_string();
                    async move {
                        handle.update(Message::Downloading(name, progress));
                        Ok(())
                    }
                })),
            )
            .await?;

            handle.update(Message::Unpacking(package.meta.name.to_string()));
            download.unpack().await?;

            handle.update(Message::Finished(package.meta.name.to_string()));
            handle.print(vec![
                "Installed ".green(),
                package.meta.name.to_string().bold(),
            ]);

            // Get smarter borrow checker
            drop(package);

            Ok(()) as Result<(), Error>
        }))
        .buffer_unordered(CONCURRENT_TASKS)
        .try_collect()
        .await?;

        Ok(()) as Result<(), Error>
    })
    .await??;

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

#[derive(Clone, Copy)]
enum Status {
    Downloading(f64),
    Unpacking,
}

#[derive(PartialEq, Eq)]
struct Key {
    package: String,
    added: Instant,
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.added.partial_cmp(&other.added)
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.added.cmp(&other.added)
    }
}

struct Program {
    total: usize,
    finished: usize,
    in_progress: BTreeMap<String, Status>,
}

impl Program {
    fn new(total: usize) -> Self {
        Self {
            total,
            finished: 0,
            in_progress: BTreeMap::default(),
        }
    }
}

enum Message {
    Downloading(String, f64),
    Unpacking(String),
    Finished(String),
}

impl tui::Program for Program {
    type Message = Message;

    const LINES: u16 = CONCURRENT_TASKS as u16 + 1;

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::Downloading(package, progress) => {
                self.in_progress
                    .insert(package, Status::Downloading(progress));
            }
            Message::Unpacking(package) => {
                self.in_progress.insert(package, Status::Unpacking);
            }
            Message::Finished(package) => {
                self.finished += 1;
                self.in_progress.remove(&package);
            }
        }
    }

    fn draw(&self, frame: &mut tui::Frame) {
        let layout = Layout::new()
            .direction(Direction::Vertical)
            .constraints(
                (0..Self::LINES)
                    .map(|_| Constraint::Length(1))
                    .collect::<Vec<_>>(),
            )
            .split(frame.size());

        self.in_progress
            .iter()
            .enumerate()
            .for_each(|(i, (package, status))| {
                let paragraph = Paragraph::new(Line::from(vec![
                    match status {
                        Status::Downloading(_) => "Downloading ".blue(),
                        Status::Unpacking => "Unpacking ".yellow(),
                    },
                    package.clone().bold(),
                ]));
                let pct = match status {
                    Status::Downloading(pct) => *pct,
                    Status::Unpacking => 0.0,
                };

                let layout = Layout::new()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(13), Constraint::Max(u16::MAX)])
                    .split(layout[i]);

                frame.render_widget(progress(pct as f32, progress::Fill::UpAcross, 5), layout[0]);
                frame.render_widget(paragraph, layout[1]);
            });

        frame.render_widget(
            progress(
                self.finished as f32 / self.total as f32,
                progress::Fill::UpAcross,
                20,
            ),
            layout[Self::LINES as usize - 1],
        );
    }
}
