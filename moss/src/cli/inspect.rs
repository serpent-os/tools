// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{arg, ArgMatches, Command};
use reqwest::header::Entry;
use std::fs::File;
use std::path::PathBuf;
use stone::payload::layout::Entry as LayoutEntry;
use stone::payload::meta;
use stone::read::Payload;
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
pub fn handle(args: &ArgMatches) -> Result<(), Error> {
    let paths = args
        .get_many::<PathBuf>("PATH")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    // Process each input path in order.
    for path in paths {
        let rdr = File::open(path).map_err(Error::IO)?;
        let mut reader = stone::read(rdr).map_err(Error::Format)?;

        // Grab the header version
        print!(
            "{path:?} = stone container version {:?}",
            reader.header.version()
        );

        for result in reader.payloads()? {
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
                            _ => println!("{:width$} : {:?}", name, record, width = COLUMN_WIDTH),
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
                        LayoutEntry::Regular(hash, target) => {
                            println!("    - /usr/{} - [Regular] {:02x}", target, hash)
                        }
                        LayoutEntry::Directory(target) => {
                            println!("    - /usr/{} [Directory]", target)
                        }
                        LayoutEntry::Symlink(source, target) => {
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
    #[error("Read failure")]
    IO(#[from] std::io::Error),

    #[error("Format failure")]
    Format(#[from] stone::read::Error),
}
