// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{arg, ArgMatches, Command};
use futures::StreamExt;
use moss::stone;
use moss::stone::payload::layout;
use moss::stone::payload::meta;
use moss::stone::read::Payload;
use std::path::PathBuf;
use thiserror::Error;

const COLUMN_WIDTH: usize = 20;

pub fn command() -> Command {
    Command::new("inspect")
        .about("Examine raw stone files")
        .long_about("Show detailed (debug) information on a local `.stone` file")
        .arg(arg!(<PATH> ... "files to inspect").value_parser(clap::value_parser!(PathBuf)))
}

///
/// Inspect the given .stone files and print results
///
pub async fn handle(args: &ArgMatches) -> Result<(), Error> {
    let paths = args
        .get_many::<PathBuf>("PATH")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

    inspect(paths).await
}

async fn inspect(paths: Vec<PathBuf>) -> Result<(), Error> {
    // Process each input path in order.
    for path in paths {
        let (header, mut payloads) = stone::stream_payloads(&path).await?;

        // Grab the header version
        print!("{path:?} = stone container version {:?}", header.version());

        while let Some(result) = payloads.next().await {
            let payload = result?;

            let mut layouts = vec![];

            // Grab deps/providers
            let mut deps = vec![];
            let mut provs = vec![];

            match payload {
                Payload::Layout(l) => layouts = l,
                Payload::Meta(meta) => {
                    println!();

                    for record in meta {
                        let name = format!("{:?}", record.tag);

                        match &record.kind {
                            meta::Kind::Provider(k, p) => deps.push(format!("{}({})", k, p)),
                            meta::Kind::Dependency(k, d) => provs.push(format!("{}({})", k, d)),
                            meta::Kind::String(s) => {
                                println!("{:width$} : {}", name, s, width = COLUMN_WIDTH)
                            }
                            meta::Kind::Int64(i) => {
                                println!("{:width$} : {}", name, i, width = COLUMN_WIDTH)
                            }
                            meta::Kind::Uint64(i) => {
                                println!("{:width$} : {}", name, i, width = COLUMN_WIDTH)
                            }
                            _ => {
                                println!("{:width$} : {:?}", name, record, width = COLUMN_WIDTH)
                            }
                        }
                    }
                }
                _ => {}
            }

            if !deps.is_empty() {
                println!("\n{:width$} :", "Dependencies", width = COLUMN_WIDTH);
                for dep in deps {
                    println!("    - {dep}");
                }
            }
            if !provs.is_empty() {
                println!("\n{:width$} :", "Providers", width = COLUMN_WIDTH);
                for prov in provs {
                    println!("    - {prov}");
                }
            }

            if !layouts.is_empty() {
                println!("\n{:width$} :", "Layout entries", width = COLUMN_WIDTH);
                for layout in layouts {
                    match layout.entry {
                        layout::Entry::Regular(hash, target) => {
                            println!("    - /usr/{} - [Regular] {:02x}", target, hash)
                        }
                        layout::Entry::Directory(target) => {
                            println!("    - /usr/{} [Directory]", target)
                        }
                        layout::Entry::Symlink(source, target) => {
                            println!("    - /usr/{} -> {} [Symlink]", target, source)
                        }
                        _ => unreachable!(),
                    };
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("io")]
    IO(#[from] std::io::Error),

    #[error("stone format")]
    Format(#[from] stone::read::Error),
}
