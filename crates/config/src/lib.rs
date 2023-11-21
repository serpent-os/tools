// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fmt,
    path::{Path, PathBuf},
};

use futures::StreamExt;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::{fs, io};
use tokio_stream::wrappers::ReadDirStream;

const EXTENSION: &str = "yaml";

pub trait Config: DeserializeOwned {
    fn domain() -> String;

    fn merge(self, other: Self) -> Self;
}

#[derive(Debug, Clone)]
pub struct Manager {
    program: String,
    scope: Scope,
}

impl Manager {
    pub fn system(root: impl Into<PathBuf>, program: impl ToString) -> Self {
        Self {
            program: program.to_string(),
            scope: Scope::System(root.into()),
        }
    }

    pub fn user(program: impl ToString) -> Option<Self> {
        Some(Self {
            program: program.to_string(),
            scope: Scope::User(dirs::config_dir()?),
        })
    }

    pub async fn load<T: Config>(&self) -> Option<T> {
        let domain = T::domain();

        let mut configs = vec![];

        let searches = match &self.scope {
            // System we search / merge all base file / .d files
            // from vendor then admin
            Scope::System(root) => vec![
                (
                    Entry::File,
                    Search::System {
                        root,
                        base: Base::Vendor,
                    },
                ),
                (
                    Entry::Directory,
                    Search::System {
                        root,
                        base: Base::Vendor,
                    },
                ),
                (
                    Entry::File,
                    Search::System {
                        root,
                        base: Base::Admin,
                    },
                ),
                (
                    Entry::Directory,
                    Search::System {
                        root,
                        base: Base::Admin,
                    },
                ),
            ],
            // User we only get configs from directory
            Scope::User(root) => vec![(Entry::Directory, Search::Home(root))],
        };

        for (entry, search) in searches {
            for path in enumerate_paths(entry, search, &self.program, &domain).await {
                if let Some(config) = read_config(path).await {
                    configs.push(config);
                }
            }
        }

        configs.into_iter().reduce(T::merge)
    }

    pub async fn save<T: Config + Serialize>(
        &self,
        name: impl fmt::Display,
        config: &T,
    ) -> Result<(), SaveError> {
        let domain = T::domain();

        let search = match &self.scope {
            Scope::System(root) => Search::System {
                root,
                base: Base::Admin,
            },
            Scope::User(root) => Search::Home(root),
        };
        let dir = search.dir(&self.program, &domain);

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
    entry: Entry,
    search: Search<'_>,
    program: &str,
    domain: &str,
) -> Vec<PathBuf> {
    match entry {
        Entry::File => {
            let file = search.file(program, domain);

            if file.exists() {
                vec![file]
            } else {
                vec![]
            }
        }
        Entry::Directory => {
            if let Ok(read_dir) = fs::read_dir(search.dir(program, domain)).await {
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

enum Entry {
    File,
    Directory,
}

enum Search<'a> {
    System { root: &'a Path, base: Base },
    Home(&'a Path),
}

impl<'a> Search<'a> {
    fn file(&self, program: &str, domain: &str) -> PathBuf {
        match self {
            Search::System { root, base } => root.join(base.path()).join(program),
            Search::Home(root) => root.join(program),
        }
        .join(format!("{domain}.{EXTENSION}"))
    }

    fn dir(&self, program: &str, domain: &str) -> PathBuf {
        match self {
            Search::System { root, base } => root
                .join(base.path())
                .join(program)
                .join(format!("{domain}.d")),
            Search::Home(root) => root.join(program).join("{domain}"),
        }
    }
}

#[derive(Debug, Clone)]
enum Scope {
    System(PathBuf),
    User(PathBuf),
}
