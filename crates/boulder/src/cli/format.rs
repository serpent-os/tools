// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
};

use clap::Parser;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(
    about = "Format a recipe file. If no recipe file is supplied, it is read from standard input."
)]
pub struct Command {
    #[arg(help = "Path to recipe file")]
    recipe: Option<PathBuf>,
    #[arg(
        short = 'w',
        long,
        default_value = "false",
        help = "Overwrite the recipe file in place instead of printing to standard output"
    )]
    overwrite: bool,
}

pub fn handle(command: Command) -> Result<(), Error> {
    if command.overwrite && command.recipe.is_none() {
        return Err(Error::OverwriteRecipeRequired);
    }

    let input = if let Some(recipe) = &command.recipe {
        fs::read(recipe).map_err(Error::Read)?
    } else {
        let mut bytes = vec![];
        io::stdin()
            .lock()
            .read_to_end(&mut bytes)
            .map_err(Error::Read)?;
        bytes
    };

    let value: serde_yaml::Value = serde_yaml::from_slice(&input).map_err(Error::Deser)?;

    let formatted = yaml::format(&value)?;

    if command.overwrite {
        let recipe = command.recipe.expect("checked above");
        fs::write(&recipe, formatted.as_bytes()).map_err(Error::Write)?;
        println!("{} updated", recipe.display())
    } else {
        print!("{formatted}");
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Recipe file must be provided to use -w/--overwrite")]
    OverwriteRecipeRequired,
    #[error("yaml format")]
    YamlFormat(#[from] yaml::format::Error),
    #[error("reading recipe")]
    Read(#[source] io::Error),
    #[error("writing recipe")]
    Write(#[source] io::Error),
    #[error("deserializing recipe")]
    Deser(#[from] serde_yaml::Error),
}
