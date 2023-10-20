// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::HashMap, fmt, path::Path};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    fs::File,
    io::{self, AsyncWriteExt},
};
use url::Url;

use crate::{db::meta, request, Config};

pub use self::manager::Manager;

pub mod manager;

/// A unique [`Repository`] identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Id(String);

impl Id {
    pub fn new(identifier: String) -> Self {
        Self(
            identifier
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect(),
        )
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Repository configuration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub description: String,
    pub uri: Url,
    pub priority: Priority,
}

/// An active repository that has been
/// fetched and cached to a meta database
#[derive(Debug, Clone)]
pub struct Active {
    pub id: Id,
    pub repository: Repository,
    pub db: meta::Database,
}

/// The selection priority of a [`Repository`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Priority(u64);

impl Priority {
    pub fn new(priority: u64) -> Self {
        Self(priority)
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Priority> for u64 {
    fn from(priority: Priority) -> Self {
        priority.0
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0).reverse()
    }
}

/// A map of repositories
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Map(HashMap<Id, Repository>);

impl Map {
    pub fn with(items: impl IntoIterator<Item = (Id, Repository)>) -> Self {
        Self(items.into_iter().collect())
    }

    pub fn get(&self, id: &Id) -> Option<&Repository> {
        self.0.get(id)
    }

    pub fn add(&mut self, id: Id, repo: Repository) {
        self.0.insert(id, repo);
    }
}

impl IntoIterator for Map {
    type Item = (Id, Repository);
    type IntoIter = std::collections::hash_map::IntoIter<Id, Repository>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Config for Map {
    fn domain() -> String {
        "repo".into()
    }

    fn merge(self, other: Self) -> Self {
        Self(self.0.into_iter().chain(other.0).collect())
    }
}

async fn fetch_index(url: Url, out_path: impl AsRef<Path>) -> Result<(), FetchError> {
    let mut stream = request::get(url).await?;

    let mut out = File::create(out_path).await?;

    while let Some(chunk) = stream.next().await {
        out.write_all(&chunk?).await?;
    }

    out.flush().await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("request")]
    Request(#[from] request::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
