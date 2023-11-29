// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
};

use futures::{future::BoxFuture, stream, FutureExt, StreamExt, TryStreamExt};
use nix::unistd::{linkat, LinkatFlags};
use sha2::{Digest, Sha256};
use stone_recipe::Recipe;
use thiserror::Error;
use tokio::fs::{copy, read_dir, read_link, remove_dir_all, symlink};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use url::Url;

use crate::{env, Cache};

/// Cache all upstreams from the provided [`Recipe`] and make them available
/// in the guest [`Cache::upstreams`] folder.
pub async fn sync(recipe: &Recipe, cache: &Cache) -> Result<(), Error> {
    let upstreams = recipe
        .upstreams
        .iter()
        .cloned()
        .map(Upstream::from_recipe)
        .collect::<Result<Vec<_>, _>>()?;

    let cached = stream::iter(upstreams)
        .map(|upstream| async move { upstream.fetch(cache).await })
        .buffer_unordered(moss::environment::MAX_NETWORK_CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?;

    let upstream_target = cache.guest_host_path(&cache.upstreams());
    env::ensure_dir_exists(&upstream_target)?;

    for upstream in cached {
        match upstream {
            Cached::Plain { name, path } => {
                let target = upstream_target.join(&name);

                // Attempt hard link
                let link_result = linkat(None, &path, None, &target, LinkatFlags::NoSymlinkFollow);

                // Copy instead
                if link_result.is_err() {
                    copy(&path, &target).await?;
                }
            }
            Cached::Git { name, path } => {
                let target = upstream_target.join(&name);
                copy_dir(&path, &target).await?;
            }
        }
    }

    Ok(())
}

enum Cached {
    Plain { name: String, path: PathBuf },
    Git { name: String, path: PathBuf },
}

#[derive(Debug, Clone)]
pub enum Upstream {
    Plain(Plain),
    Git(Git),
}

impl Upstream {
    pub fn path(&self, cache: &Cache) -> PathBuf {
        match self {
            Upstream::Plain(plain) => plain.path(cache),
            Upstream::Git(git) => git.final_path(cache),
        }
    }

    pub fn from_recipe(upstream: stone_recipe::Upstream) -> Result<Self, Error> {
        match upstream {
            stone_recipe::Upstream::Plain {
                uri,
                hash,
                rename,
                strip_dirs,
                unpack,
                unpack_dir,
            } => Ok(Self::Plain(Plain {
                uri,
                hash: hash.parse()?,
                rename,
                strip_dirs,
                unpack,
                unpack_dir,
            })),
            stone_recipe::Upstream::Git {
                uri,
                ref_id,
                clone_dir,
                staging,
            } => Ok(Self::Git(Git {
                uri,
                ref_id,
                clone_dir,
                staging,
            })),
        }
    }

    async fn fetch(&self, cache: &Cache) -> Result<Cached, Error> {
        match self {
            Upstream::Plain(plain) => plain.fetch(cache).await,
            Upstream::Git(git) => git.fetch(cache).await,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Hash(String);

impl FromStr for Hash {
    type Err = ParseHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() < 5 {
            return Err(ParseHashError::TooShort(s.to_string()));
        }

        Ok(Self(s.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ParseHashError {
    #[error("hash too short: {0}")]
    TooShort(String),
}

#[derive(Debug, Clone)]
pub struct Plain {
    uri: Url,
    hash: Hash,
    rename: Option<String>,
    strip_dirs: u8,
    unpack: bool,
    unpack_dir: PathBuf,
}

impl Plain {
    fn name(&self) -> &str {
        if let Some(name) = &self.rename {
            name
        } else {
            self.uri.path().split('/').last().unwrap_or_default()
        }
    }

    fn path(&self, cache: &Cache) -> PathBuf {
        // Type safe guaranteed to be >= 5 bytes
        let hash = &self.hash.0;

        let parent = cache
            .upstreams()
            .host
            .join("fetched")
            .join(&hash[..5])
            .join(&hash[hash.len() - 5..]);

        let _ = env::ensure_dir_exists(&parent);

        parent.join(hash)
    }

    async fn fetch(&self, cache: &Cache) -> Result<Cached, Error> {
        use moss::request;
        use tokio::fs;

        let name = self.name();
        let path = self.path(cache);

        if path.exists() {
            return Ok(Cached::Plain {
                name: name.to_string(),
                path,
            });
        }

        let mut stream = request::get(self.uri.clone()).await?;

        let mut hasher = Sha256::new();
        let mut out = fs::File::create(&path).await?;

        while let Some(chunk) = stream.next().await {
            let bytes = &chunk?;
            hasher.update(bytes);
            out.write_all(bytes).await?;
        }

        out.flush().await?;

        let hash = hex::encode(hasher.finalize());

        if hash != self.hash.0 {
            fs::remove_file(&path).await?;

            return Err(Error::HashMismatch {
                name: name.to_string(),
                expected: self.hash.0.clone(),
                got: hash,
            });
        }

        Ok(Cached::Plain {
            name: name.to_string(),
            path,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Git {
    uri: Url,
    ref_id: String,
    clone_dir: Option<PathBuf>,
    staging: bool,
}

impl Git {
    fn name(&self) -> &str {
        self.uri.path().split('/').last().unwrap_or_default()
    }

    fn final_path(&self, cache: &Cache) -> PathBuf {
        let path = self.uri.path();
        let relative_path = path.strip_prefix('/').unwrap_or(path);
        let parent = cache.upstreams().host.join("git");

        let _ = env::ensure_dir_exists(&parent);

        parent.join(relative_path)
    }

    fn staging_path(&self, cache: &Cache) -> PathBuf {
        let path = self.uri.path();
        let relative_path = path.strip_prefix('/').unwrap_or(path);
        let parent = cache.upstreams().host.join("staging").join("git");

        let _ = env::ensure_dir_exists(&parent);

        parent.join(relative_path)
    }

    async fn fetch(&self, cache: &Cache) -> Result<Cached, Error> {
        let clone_path = if self.staging {
            self.staging_path(cache)
        } else {
            self.final_path(cache)
        };
        let clone_path_string = clone_path.display().to_string();

        let final_path = self.final_path(cache);
        let final_path_string = final_path.display().to_string();

        if self.ref_exists(&final_path).await? {
            self.reset_to_ref(&final_path).await?;
            return Ok(Cached::Git {
                name: self.name().to_string(),
                path: final_path,
            });
        }

        let _ = remove_dir_all(&clone_path).await;
        if self.staging {
            let _ = remove_dir_all(&final_path).await;
        }

        let mut args = vec!["clone"];
        if self.staging {
            args.push("--mirror");
        }
        args.extend(["--", self.uri.as_str(), &clone_path_string]);

        self.run(&args, None).await?;

        if self.staging {
            self.run(
                &["clone", "--", &clone_path_string, &final_path_string],
                None,
            )
            .await?;
        }

        self.reset_to_ref(&final_path).await?;

        Ok(Cached::Git {
            name: self.name().to_string(),
            path: final_path,
        })
    }

    async fn ref_exists(&self, path: &Path) -> Result<bool, Error> {
        if !path.exists() {
            return Ok(false);
        }

        self.run(&["fetch"], Some(path)).await?;

        let result = self
            .run(&["cat-file", "-e", &self.ref_id], Some(path))
            .await;

        Ok(result.is_ok())
    }

    async fn reset_to_ref(&self, path: &Path) -> Result<(), Error> {
        self.run(&["reset", "--hard", &self.ref_id], Some(path))
            .await?;

        self.run(
            &[
                "submodule",
                "update",
                "--init",
                "--recursive",
                "--depth",
                "1",
                "--jobs",
                "4",
            ],
            Some(path),
        )
        .await?;

        Ok(())
    }

    async fn run(&self, args: &[&str], cwd: Option<&Path>) -> Result<(), Error> {
        let mut command = Command::new("git");

        if let Some(dir) = cwd {
            command.current_dir(dir);
        }

        let output = command.args(args).output().await?;

        if !output.status.success() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(Error::GitFailed(self.uri.clone()));
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to clone {0}")]
    GitFailed(Url),
    #[error("parse hash")]
    ParseHash(#[from] ParseHashError),
    #[error("hash mismatch for {name}, expected {expected:?} got {got:?}")]
    HashMismatch {
        name: String,
        expected: String,
        got: String,
    },
    #[error("request")]
    Request(#[from] moss::request::Error),
    #[error("io")]
    Io(#[from] io::Error),
}

fn copy_dir<'a>(source_dir: &'a Path, out_dir: &'a Path) -> BoxFuture<'a, Result<(), Error>> {
    async move {
        env::recreate_dir(out_dir)?;

        let mut contents = read_dir(&source_dir).await?;

        while let Some(entry) = contents.next_entry().await? {
            let path = entry.path();

            if let Some(file_name) = path.file_name() {
                let dest = out_dir.join(file_name);
                let meta = entry.metadata().await?;

                if meta.is_dir() {
                    copy_dir(&path, &dest).await?;
                } else if meta.is_file() {
                    copy(&path, &dest).await?;
                } else if meta.is_symlink() {
                    symlink(read_link(&path).await?, &dest).await?;
                }
            }
        }

        Ok(())
    }
    .boxed()
}
