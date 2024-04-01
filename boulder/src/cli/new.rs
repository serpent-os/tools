// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fs, path::PathBuf};

use boulder::draft::{self, Drafter};
use clap::Parser;
use futures::io;
use moss::runtime;
use thiserror::Error;
use url::Url;

#[derive(Debug, Parser)]
#[command(about = "Create skeletal stone.yaml recipe from source archive URIs")]
pub struct Command {
    #[arg(
        short,
        long,
        default_value = "./stone.yaml",
        help = "Location to output generated build recipe"
    )]
    output: PathBuf,
    #[arg(required = true, value_name = "URI", help = "Source archive URIs")]
    upstreams: Vec<Url>,
}

pub fn handle(command: Command) -> Result<(), Error> {
    let Command { output, upstreams } = command;

    // We use async to fetch upstreams
    let _guard = runtime::init();

    let drafter = Drafter::new(upstreams);
    let recipe = drafter.run()?;

    fs::write(&output, recipe).map_err(Error::WriteRecipe)?;

    println!("Saved recipe to {output:?}");

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("draft")]
    Draft(#[from] draft::Error),
    #[error("failed to write output file")]
    WriteRecipe(#[source] io::Error),
}
