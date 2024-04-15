// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::builder::NonEmptyStringValueParser;
use clap::{Arg, ArgMatches, Command};

use moss::client;
use moss::package::{self, Name};
use moss::{environment, Client, Installation};
use tui::pretty::{print_columns, ColumnDisplay};
use tui::Styled;

const ARG_KEYWORD: &str = "KEYWORD";
const FLAG_INSTALLED: &str = "installed";

/// Returns the Clap struct for this command.
pub fn command() -> Command {
    Command::new("search")
        .visible_alias("sr")
        .about("Search packages")
        .long_about("Search packages by looking into package names and summaries.")
        .arg(
            Arg::new(ARG_KEYWORD)
                .required(true)
                .num_args(1)
                .value_parser(NonEmptyStringValueParser::new()),
        )
        .arg(
            Arg::new(FLAG_INSTALLED)
                .short('i')
                .long("installed")
                .num_args(0)
                .help("Search among installed packages only"),
        )
}

pub fn handle(args: &ArgMatches, installation: Installation) -> Result<(), Error> {
    let keyword = args.get_one::<String>(ARG_KEYWORD).unwrap();
    let only_installed = args.get_flag(FLAG_INSTALLED);

    let client = Client::new(environment::NAME, installation)?;
    let flags = if only_installed {
        package::Flags::new().with_installed()
    } else {
        package::Flags::new().with_available()
    };

    let output: Vec<Output> = client
        .registry
        .by_keyword(keyword, flags)
        .map(|pkg| Output {
            name: pkg.meta.name,
            summary: pkg.meta.summary,
        })
        .collect();

    if output.is_empty() {
        return Ok(());
    }

    print_columns(&output, 1);

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("client")]
    Client(#[from] client::Error),
}

const COLUMN_SPACING: usize = 4;

struct Output {
    name: Name,
    summary: String,
}

impl ColumnDisplay for Output {
    fn get_display_width(&self) -> usize {
        // TODO: calculate the number of graphemes, not bytes.
        // Now we are assuming name and summary are ASCII.
        self.name.as_ref().len() + self.summary.len() + COLUMN_SPACING
    }

    fn display_column(&self, writer: &mut impl std::io::prelude::Write, _col: tui::pretty::Column, width: usize) {
        let _ = write!(
            writer,
            "{}{}{:width$}{}",
            self.name.to_string().bold(),
            " ".repeat(COLUMN_SPACING),
            " ",
            self.summary,
        );
    }
}
