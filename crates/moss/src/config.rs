// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use futures::StreamExt;
use std::{
    fmt,
    path::{Path, PathBuf},
};
use tokio::{fs, io};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio_stream::wrappers::ReadDirStream;

const EXTENSION: &str = "conf";

pub trait Config: DeserializeOwned {
    fn domain() -> String;

    fn merge(self, other: Self) -> Self;
}

pub async fn load<T: Config>(root: impl AsRef<Path>) -> Option<T> {
    let domain = T::domain();

    let mut configs = vec![];

    for (base, search) in [
        (Base::Vendor, Search::File),
        (Base::Vendor, Search::Directory),
        (Base::Admin, Search::File),
        (Base::Admin, Search::Directory),
    ] {
        for path in enumerate_paths(search, &root, base, &domain).await {
            if let Some(config) = read_config(path).await {
                configs.push(config);
            }
        }
    }

    configs.into_iter().reduce(T::merge)
}

pub async fn save<T: Config + Serialize>(
    root: impl AsRef<Path>,
    name: impl fmt::Display,
    config: &T,
) -> Result<(), SaveError> {
    let domain = T::domain();

    let dir = domain_dir(root, Base::Admin, &domain);

    fs::create_dir_all(&dir)
        .await
        .map_err(|io| SaveError::CreateDir(dir.clone(), io))?;

    let path = dir.join(format!("{name}.{EXTENSION}"));

    let serialized = serde_yaml::to_string(config)?;

    fs::write(&path, serialized)
        .await
        .map_err(|io| SaveError::Write(path, io))?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("create config dir {0:?}")]
    CreateDir(PathBuf, #[source] io::Error),
    #[error("serialize config")]
    Yaml(#[from] serde_yaml::Error),
    #[error("write config file {0:?}")]
    Write(PathBuf, #[source] io::Error),
}

async fn enumerate_paths(
    search: Search,
    root: &impl AsRef<Path>,
    base: Base,
    domain: &str,
) -> Vec<PathBuf> {
    match search {
        Search::File => {
            let file = domain_file(root, base, domain);

            if file.exists() {
                vec![file]
            } else {
                vec![]
            }
        }
        Search::Directory => {
            if let Ok(read_dir) = fs::read_dir(domain_dir(root, base, domain)).await {
                ReadDirStream::new(read_dir)
                    .filter_map(|entry| async {
                        let entry = entry.ok()?;
                        let path = entry.path();
                        let extension = path
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .unwrap_or_default();

                        if path.exists() && extension == EXTENSION {
                            Some(path)
                        } else {
                            None
                        }
                    })
                    .collect()
                    .await
            } else {
                vec![]
            }
        }
    }
}

fn domain_file(root: impl AsRef<Path>, base: Base, domain: &str) -> PathBuf {
    root.as_ref()
        .join(base.path())
        .join("moss")
        .join(format!("{domain}.{EXTENSION}"))
}

fn domain_dir(root: impl AsRef<Path>, base: Base, domain: &str) -> PathBuf {
    root.as_ref()
        .join(base.path())
        .join("moss")
        .join(format!("{domain}.{EXTENSION}.d"))
}

async fn read_config<T: Config>(path: PathBuf) -> Option<T> {
    let bytes = fs::read(path).await.ok()?;
    serde_yaml::from_slice(&bytes).ok()
}

#[derive(Clone, Copy)]
enum Base {
    Admin,
    Vendor,
}

impl Base {
    fn path(&self) -> &'static str {
        match self {
            Base::Admin => "etc",
            Base::Vendor => "usr/share",
        }
    }
}

enum Search {
    File,
    Directory,
}
