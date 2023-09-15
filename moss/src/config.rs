// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fmt,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

const EXTENSION: &str = "conf";

pub trait Config: DeserializeOwned {
    fn domain() -> String;

    fn merge(self, other: Self) -> Self;
}

pub fn load<T: Config>(root: impl AsRef<Path>) -> Option<T> {
    let domain = T::domain();

    [
        (Base::Vendor, Search::File),
        (Base::Vendor, Search::Directory),
        (Base::Admin, Search::File),
        (Base::Admin, Search::Directory),
    ]
    .into_iter()
    .flat_map(|(base, search)| enumerate_paths(search, &root, base, &domain))
    .filter_map(read_config)
    .reduce(T::merge)
}

pub fn save<T: Config + Serialize>(
    root: impl AsRef<Path>,
    name: impl fmt::Display,
    config: &T,
) -> Result<(), SaveError> {
    let domain = T::domain();

    let dir = domain_dir(root, Base::Admin, &domain);

    fs::create_dir_all(&dir).map_err(|io| SaveError::CreateDir(dir.clone(), io))?;

    let path = dir.join(format!("{name}.{EXTENSION}"));

    let serialized = serde_yaml::to_string(config)?;

    fs::write(&path, serialized).map_err(|io| SaveError::Write(path, io))?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("could not create config dir {0:?}: {1}")]
    CreateDir(PathBuf, io::Error),
    #[error("failed to serialize config as yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("failed to write config file at {0:?}: {1}")]
    Write(PathBuf, io::Error),
}

fn enumerate_paths(
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
        Search::Directory => fs::read_dir(domain_dir(root, base, domain))
            .map(|read_dir| {
                read_dir
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| {
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
            })
            .unwrap_or_default(),
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

fn read_config<T: Config>(path: PathBuf) -> Option<T> {
    let file = File::open(path).ok()?;
    serde_yaml::from_reader(file).ok()
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
