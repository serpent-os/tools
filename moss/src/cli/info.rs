// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use clap::{arg, ArgMatches, Command};
use futures::StreamExt;
use itertools::Itertools;
use moss::{
    client::{self, Client},
    package::Flags,
    Package, Provider,
};
use thiserror::Error;
use tui::Stylize;

const COLUMN_WIDTH: usize = 20;

pub fn command() -> Command {
    Command::new("info")
        .about("Query packages")
        .long_about("List detailed package information from all available sources")
        .arg(arg!(<NAME> ... "packages to query").value_parser(clap::value_parser!(String)))
}

/// For all arguments, try to match a package
pub async fn handle(args: &ArgMatches) -> Result<(), Error> {
    let pkgs = args
        .get_many::<String>("NAME")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    let root = args.get_one::<PathBuf>("root").unwrap().clone();
    let client = Client::new(root).await?;

    for pkg in pkgs {
        let lookup = Provider::from_name(&pkg).unwrap();
        let resolved = client
            .registry
            .by_provider(&lookup, Flags::NONE)
            .collect::<Vec<_>>()
            .await;
        if resolved.is_empty() {
            return Err(Error::NotFound(pkg));
        }
        for candidate in resolved {
            print_package(&candidate);
        }
    }

    Ok(())
}

/// Print the title for each metadata section
fn print_titled(title: &'static str) {
    let display_width = COLUMN_WIDTH - title.len();
    print!("{}{:width$} ", title.bold(), " ", width = display_width);
}

/// HAX: Printing a paragraph by line breaks.
/// TODO: Split into proper paragraphs - limited to num columns in tty
fn print_paragraph(p: &str) {
    for (index, line) in p.lines().enumerate() {
        match index {
            0 => println!("{}", line),
            _ => println!("{:width$} {}", " ", line.dim(), width = COLUMN_WIDTH),
        }
    }
}

/// Pretty print a package
fn print_package(pkg: &Package) {
    print_titled("Name");
    println!("{}", pkg.meta.name);
    print_titled("Version");
    println!("{}", pkg.meta.version_identifier);
    print_titled("Homepage");
    println!("{}", pkg.meta.homepage);
    print_titled("Summary");
    println!("{}", pkg.meta.summary);
    print_titled("Description");
    print_paragraph(&pkg.meta.description);
    if !pkg.meta.dependencies.is_empty() {
        print_titled("Dependencies");
        let deps = pkg
            .meta
            .dependencies
            .iter()
            .map(|d| d.to_string())
            .sorted()
            .join("\n");
        print_paragraph(&deps);
    }
    if !pkg.meta.providers.is_empty() {
        print_titled("Providers");
        let provs = pkg
            .meta
            .providers
            .iter()
            .map(|p| p.to_string())
            .sorted()
            .join("\n");
        print_paragraph(&provs);
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("No such package {0}")]
    NotFound(String),

    #[error("client")]
    Client(#[from] client::Error),
}
