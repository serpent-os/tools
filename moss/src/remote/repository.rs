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

use crate::Config;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub description: String,
    pub url: Url,
    pub priority: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Map(HashMap<Id, Repository>);

impl Map {
    pub fn get(&self, id: &Id) -> Option<&Repository> {
        self.0.get(id)
    }

    pub fn add(&mut self, id: Id, repo: Repository) {
        self.0.insert(id, repo);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Id, &Repository)> {
        self.0.iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &Id> {
        self.0.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &Repository> {
        self.0.values()
    }
}

impl Config for Map {
    fn domain() -> String {
        "repos".into()
    }

    fn merge(self, other: Self) -> Self {
        Self(self.0.into_iter().chain(other.0.into_iter()).collect())
    }
}

pub async fn fetch_index(url: Url, out_path: impl AsRef<Path>) -> Result<(), FetchError> {
    let resp = reqwest::get(url).await?;

    let mut out = File::create(out_path).await?;

    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        out.write_all(&chunk?).await?;
    }

    out.flush().await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}
