// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
};

use boulder::{
    architecture,
    draft::{self, Drafter},
    macros, recipe, Env, Macros,
};
use clap::Parser;
use futures::StreamExt;
use itertools::Itertools;
use moss::{request, runtime};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tui::{
    pretty::{self, ColumnDisplay},
    Styled,
};
use url::Url;

#[derive(Debug, Parser)]
#[command(about = "Utilities to create and manipulate stone recipe files")]
pub struct Command {
    #[command(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    #[command(about = "Create skeletal stone.yaml recipe from source archive URIs")]
    New {
        #[arg(
            short,
            long,
            default_value = "./stone.yaml",
            help = "Location to output generated build recipe"
        )]
        output: PathBuf,
        #[arg(required = true, value_name = "URI", help = "Source archive URIs")]
        upstreams: Vec<Url>,
    },
    #[command(about = "Update a recipe file")]
    Update {
        #[arg(short, long, required = true, help = "Update version")]
        version: String,
        #[arg(
            short,
            long = "upstream",
            required = true,
            value_parser = parse_upstream,
            help = "Update upstream source, can be passed multiple times. Applied in same order as defined in recipe file.",
            long_help = "Update upstream source, can be passed multiple times. Applied in same order as defined in recipe file.\n\nExample: -u \"https://some.plan/file.tar.gz\" -u \"git|v1.1\"",
        )]
        upstreams: Vec<Upstream>,
        #[arg(help = "Path to recipe file, otherwise read from standard input")]
        recipe: Option<PathBuf>,
        #[arg(
            short = 'w',
            long,
            default_value = "false",
            help = "Overwrite the recipe file in place instead of printing to standard output"
        )]
        overwrite: bool,
    },
    #[command(about = "Print macro definitions")]
    Macros {
        #[arg(name = "macro", help = "Print definition and example for the provided macro")]
        _macro: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub enum Upstream {
    Plain(Url),
    Git(String),
}

fn parse_upstream(s: &str) -> Result<Upstream, String> {
    match s.strip_prefix("git|") {
        Some(rev) => Ok(Upstream::Git(rev.to_string())),
        None => Ok(Upstream::Plain(s.parse::<Url>().map_err(|e| e.to_string())?)),
    }
}

pub fn handle(command: Command, env: Env) -> Result<(), Error> {
    match command.subcommand {
        Subcommand::New { output, upstreams } => new(output, upstreams),
        Subcommand::Update {
            recipe,
            overwrite,
            version,
            upstreams,
        } => update(recipe, overwrite, version, upstreams),
        Subcommand::Macros { _macro } => macros(_macro, env),
    }
}

fn new(output: PathBuf, upstreams: Vec<Url>) -> Result<(), Error> {
    // We use async to fetch upstreams
    let _guard = runtime::init();

    let drafter = Drafter::new(upstreams);
    let recipe = drafter.run()?;

    fs::write(&output, recipe).map_err(Error::Write)?;

    println!("Saved recipe to {output:?}");

    Ok(())
}

fn update(recipe: Option<PathBuf>, overwrite: bool, version: String, upstreams: Vec<Upstream>) -> Result<(), Error> {
    if overwrite && recipe.is_none() {
        return Err(Error::OverwriteRecipeRequired);
    }

    let input = if let Some(recipe_path) = &recipe {
        let path = recipe::resolve_path(recipe_path).map_err(Error::ResolvePath)?;

        fs::read_to_string(path).map_err(Error::Read)?
    } else {
        let mut bytes = vec![];
        io::stdin().lock().read_to_end(&mut bytes).map_err(Error::Read)?;
        String::from_utf8(bytes)?
    };

    // Parsed allows us to access known values in a type safe way
    let parsed: recipe::Parsed = serde_yaml::from_str(&input)?;
    // Value allows us to access map keys in their original form
    let value: serde_yaml::Value = serde_yaml::from_str(&input)?;

    #[derive(Debug)]
    enum Update {
        Release(u64),
        Version(String),
        PlainUpstream(usize, serde_yaml::Value, Url),
        GitUpstream(usize, serde_yaml::Value, String),
    }

    let mut updates = vec![Update::Version(version), Update::Release(parsed.source.release + 1)];

    for (i, (original, update)) in parsed.upstreams.iter().zip(upstreams).enumerate() {
        match (original, update) {
            (stone_recipe::Upstream::Plain { .. }, Upstream::Git(_)) => {
                return Err(Error::UpstreamMismatch(i, "Plain", "Git"))
            }
            (stone_recipe::Upstream::Git { .. }, Upstream::Plain(_)) => {
                return Err(Error::UpstreamMismatch(i, "Git", "Plain"))
            }
            (stone_recipe::Upstream::Plain { .. }, Upstream::Plain(new_uri)) => {
                let key = value["upstreams"][i]
                    .as_mapping()
                    .and_then(|map| map.keys().next())
                    .cloned();
                if let Some(key) = key {
                    updates.push(Update::PlainUpstream(i, key, new_uri));
                }
            }
            (stone_recipe::Upstream::Git { .. }, Upstream::Git(new_ref)) => {
                let key = value["upstreams"][i]
                    .as_mapping()
                    .and_then(|map| map.keys().next())
                    .cloned();
                if let Some(key) = key {
                    updates.push(Update::GitUpstream(i, key, new_ref))
                }
            }
        }
    }

    // Needed to fetch
    let _guard = runtime::init();

    // Add all update operations
    let mut updater = yaml::Updater::new();
    for update in updates {
        match update {
            Update::Release(release) => {
                updater.update_value(release, |root| root / "release");
            }
            Update::Version(version) => {
                updater.update_value(version, |root| root / "version");
            }
            Update::PlainUpstream(i, key, new_uri) => {
                let hash = runtime::block_on(fetch_hash(new_uri.clone()))?;

                let path = |root| root / "upstreams" / i / key.as_str().unwrap_or_default();

                // Update hash as either scalar or inner map "hash" value
                updater.update_value(&hash, path);
                updater.update_value(&hash, |root| path(root) / "hash");
                // Update from old to new uri
                updater.update_key(new_uri, path);
            }
            Update::GitUpstream(i, key, new_ref) => {
                let path = |root| root / "upstreams" / i / key.as_str().unwrap_or_default();

                // Update ref as either scalar or inner map "ref" value
                updater.update_value(&new_ref, path);
                updater.update_value(&new_ref, |root| path(root) / "ref");
            }
        }
    }

    // Apply updates
    let updated = updater.apply(input);

    if overwrite {
        let recipe = recipe.expect("checked above");
        fs::write(&recipe, updated.as_bytes()).map_err(Error::Write)?;
        println!("{} updated", recipe.display())
    } else {
        print!("{updated}");
    }

    Ok(())
}

async fn fetch_hash(uri: Url) -> Result<String, Error> {
    let mut stream = request::get(uri).await?;

    let mut hasher = Sha256::new();
    // Discard bytes
    let mut out = tokio::io::sink();

    while let Some(chunk) = stream.next().await {
        let bytes = &chunk?;
        hasher.update(bytes);
        out.write_all(bytes).await.map_err(Error::FetchIo)?;
    }

    out.flush().await.map_err(Error::FetchIo)?;

    let hash = hex::encode(hasher.finalize());

    Ok(hash)
}

fn macros(_macro: Option<String>, env: Env) -> Result<(), Error> {
    let macros = Macros::load(&env)?;

    let mut items = macros
        .actions
        .iter()
        .flat_map(|m| {
            m.actions.iter().map(|action| PrintMacro {
                name: format!("%{}", action.key),
                // Multi-line strings need to be in `example`
                description: action.value.description.lines().next().unwrap_or_default(),
                example: action.value.example.as_deref(),
            })
        })
        .sorted()
        .collect::<Vec<_>>();

    let mut definitions = vec![];
    for arch in ["base", &architecture::host().to_string()] {
        if let Some(macros) = macros.arch.get(arch) {
            definitions.extend(macros.definitions.iter().map(|def| PrintMacro {
                name: format!("%({})", def.key),
                description: &def.value,
                example: None,
            }));
        }
    }
    definitions.sort();
    definitions.dedup();

    items.extend(definitions);

    match _macro {
        Some(name) => {
            if let Some(action) = items
                .into_iter()
                .find(|a| a.name == format!("%{name}") || a.name == format!("%({name})"))
            {
                println!("{} - {}", action.name.bold(), action.description);

                if let Some(example) = action.example {
                    println!("\n{}", "Example:".bold());
                    for line in example.lines() {
                        println!("  {line}");
                    }
                }
            } else {
                return Err(Error::MacroNotFound(name));
            }
        }
        None => {
            pretty::print_columns(&items, 1);
        }
    }

    Ok(())
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct PrintMacro<'a> {
    name: String,
    description: &'a str,
    example: Option<&'a str>,
}

impl<'a> ColumnDisplay for PrintMacro<'a> {
    fn get_display_width(&self) -> usize {
        self.name.len()
    }

    fn display_column(&self, writer: &mut impl io::prelude::Write, _col: pretty::Column, width: usize) {
        let _ = write!(
            writer,
            "{}{}  {}",
            self.name.clone().bold(),
            " ".repeat(width),
            self.description,
        );
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Recipe file must be provided to use -w/--overwrite")]
    OverwriteRecipeRequired,
    #[error("Mismatch for upstream[{0}], expected {1} got {2}")]
    UpstreamMismatch(usize, &'static str, &'static str),
    #[error("load macros")]
    LoadMacros(#[from] macros::Error),
    #[error("Macro doesn't exist: {0}")]
    MacroNotFound(String),
    #[error("resolve recipe path")]
    ResolvePath(#[source] recipe::Error),
    #[error("reading recipe")]
    Read(#[source] io::Error),
    #[error("writing recipe")]
    Write(#[source] io::Error),
    #[error("deserializing recipe")]
    Deser(#[from] serde_yaml::Error),
    #[error("fetch upstream")]
    Fetch(#[from] request::Error),
    #[error("fetch upstream")]
    FetchIo(#[source] io::Error),
    #[error("invalid utf-8 input")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("draft")]
    Draft(#[from] draft::Error),
}
