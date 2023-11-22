// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::error::Error;

use tui::Stylize;

mod cli;

#[tokio::main]
async fn main() {
    if let Err(error) = cli::process().await {
        report_error(error);
        std::process::exit(1);
    }
}

fn report_error(error: cli::Error) {
    let sources = sources(&error);
    let error = sources.join(": ");
    eprintln!("{}: {error}", "Error".red());
}

fn sources(error: &cli::Error) -> Vec<String> {
    let mut sources = vec![error.to_string()];
    let mut source = error.source();
    while let Some(error) = source.take() {
        sources.push(error.to_string());
        source = error.source();
    }
    sources
}
