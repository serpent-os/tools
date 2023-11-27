// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashMap;

use boulder::{client, profile, Client, Profile};
use clap::Parser;
use itertools::Itertools;
use moss::{repository, Repository};
use thiserror::Error;
use url::Url;

use super::Global;

#[derive(Debug, Parser)]
#[command(about = "Manage boulder profiles")]
pub struct Command {
    #[command(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    #[command(about = "List profiles")]
    List,
    #[command(about = "Add a new profile")]
    Add {
        #[arg(help = "profile name")]
        name: String,
        #[arg(
            short = 'r',
            long = "repo",
            required = true,
            help = "profile repositories",
            value_parser = parse_repository,
            help = "repository to add to profile, can be passed multiple times",
            long_help = "repository to add to profile\n\nExample: --repo name=volatile,uri=https://dev.serpentos.com/volatile/x86_64/stone.index,priority=100"
        )]
        repos: Vec<(repository::Id, Repository)>,
    },
    #[command(about = "Update a profiles repositories")]
    Update {
        #[arg(short, long)]
        profile: profile::Id,
    },
}

/// Parse a single key-value pair
fn parse_repository(s: &str) -> Result<(repository::Id, Repository), String> {
    let key_values = s
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .collect::<HashMap<_, _>>();

    let id = repository::Id::new(key_values.get("name").ok_or("missing name")?.to_string());
    let uri = key_values
        .get("uri")
        .ok_or("missing uri")?
        .parse::<Url>()
        .map_err(|e| e.to_string())?;
    let priority = key_values
        .get("priority")
        .map(|p| p.parse::<u64>())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    Ok((
        id,
        Repository {
            description: String::default(),
            uri,
            priority: repository::Priority::new(priority),
        },
    ))
}

pub fn handle(command: Command, global: Global) -> Result<(), Error> {
    let Global {
        config_dir,
        cache_dir,
        moss_root,
    } = global;

    let client = Client::new(config_dir, cache_dir, moss_root)?;

    match command.subcommand {
        Subcommand::List => list(client),
        Subcommand::Add { name, repos } => add(client, name, repos),
        Subcommand::Update { profile } => update(client, &profile),
    }
}

pub fn list(client: Client) -> Result<(), Error> {
    if client.profiles.is_empty() {
        println!("No profiles have been configured yet");
        return Ok(());
    }

    for (id, profile) in client.profiles.iter() {
        println!("{id}:");

        for (id, repo) in profile
            .collections
            .iter()
            .sorted_by(|(_, a), (_, b)| a.priority.cmp(&b.priority).reverse())
        {
            println!(" - {} = {} [{}]", id, repo.uri, repo.priority);
        }
    }

    Ok(())
}

pub fn add(
    mut client: Client,
    name: String,
    repos: Vec<(repository::Id, Repository)>,
) -> Result<(), Error> {
    let id = profile::Id::new(name);

    client.save_profile(
        id.clone(),
        Profile {
            collections: repository::Map::with(repos),
        },
    )?;

    update(client, &id)?;

    println!("Profile \"{id}\" has been added");

    Ok(())
}

pub fn update(client: Client, profile: &profile::Id) -> Result<(), Error> {
    let repos = client.repositories(profile)?.clone();

    client.block_on(async {
        let mut moss_client = moss::Client::new("boulder", &client.moss_dir)
            .await?
            .explicit_repositories(repos)
            .await?;
        moss_client.refresh_repositories().await?;

        Ok(()) as Result<(), Error>
    })?;

    Ok(())
}
#[derive(Debug, Error)]
pub enum Error {
    #[error("client")]
    Client(#[from] client::Error),
    #[error("config")]
    Config(#[from] config::SaveError),
    #[error("moss client")]
    MossClient(#[from] moss::client::Error),
}
