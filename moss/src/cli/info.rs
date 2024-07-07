// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{arg, ArgMatches, Command};
use itertools::Itertools;
use moss::{
    client::{self, Client},
    environment,
    package::Flags,
    Installation, Package, Provider,
};
use stone::payload::layout;
use thiserror::Error;
use tui::Styled;
use vfs::tree::BlitFile;

const COLUMN_WIDTH: usize = 20;

pub fn command() -> Command {
    Command::new("info")
        .about("Query packages")
        .long_about("List detailed package information from all available sources")
        .arg_required_else_help(true)
        .arg(arg!(<NAME> ... "Packages to query").value_parser(clap::value_parser!(String)))
        .arg(arg!(-f --files ... "Show files provided by package").action(clap::ArgAction::SetTrue)) 
}

/// For all arguments, try to match a package
pub fn handle(args: &ArgMatches, installation: Installation) -> Result<(), Error> {
    let pkgs = args
        .get_many::<String>("NAME")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    let show_files = args.get_flag("files");

    let client = Client::new(environment::NAME, installation)?;

    for pkg in pkgs {
        let lookup = Provider::from_name(&pkg).unwrap();
        let resolved = client
            .registry
            .by_provider(&lookup, Flags::default())
            .collect::<Vec<_>>();
        if resolved.is_empty() {
            return Err(Error::NotFound(pkg));
        }
        for candidate in resolved {
            print_package(&candidate);

            if candidate.flags.installed && show_files {
                let vfs = client.vfs([&candidate.id])?;
                print_files(vfs);
            }
            println!();
        }
    }

    Ok(())
}

/// Print the title for each metadata section
fn print_titled(title: &'static str) {
    let display_width = COLUMN_WIDTH - title.len();
    print!("{}{:width$} ", title.bold(), " ", width = display_width);
}

/// Ugly hack: Printing a paragraph by line breaks.
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
        let deps = pkg.meta.dependencies.iter().map(|d| d.to_string()).sorted().join("\n");
        print_paragraph(&deps);
    }
    if !pkg.meta.providers.is_empty() {
        print_titled("Providers");
        let provs = pkg.meta.providers.iter().map(|p| p.to_string()).sorted().join("\n");
        print_paragraph(&provs);
    }
}

fn print_files(vfs: vfs::Tree<client::PendingFile>) {
    let files = vfs
        .iter()
        .filter_map(|file| {
            if matches!(file.kind(), vfs::tree::Kind::Directory) {
                return None;
            }

            let path = file.path();
            let meta = match file.layout.entry {
                layout::Entry::Regular(hash, _) => Some(format!(" ({hash:2x})")),
                layout::Entry::Symlink(source, _) => Some(format!(" -> {source}")),
                _ => None,
            };

            Some((path, meta))
        })
        .collect::<Vec<_>>();

    if files.is_empty() {
        return;
    }

    print_titled("Files");
    println!();
    for (path, meta) in files {
        println!("  {path}{}", meta.unwrap_or_default().dim());
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("No such package {0}")]
    NotFound(String),
    #[error("client")]
    Client(#[from] client::Error),
}
