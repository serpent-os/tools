// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    io,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use futures::{stream, StreamExt, TryStreamExt};
use nix::unistd::{linkat, LinkatFlags};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::fs::{copy, remove_dir_all};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tui::{MultiProgress, ProgressBar, ProgressStyle, Stylize};
use url::Url;

use crate::{util, Paths, Recipe};

/// Cache all upstreams from the provided [`Recipe`] and make them available
/// in the guest rootfs.
pub async fn sync(recipe: &Recipe, paths: &Paths) -> Result<(), Error> {
    let upstreams = recipe
        .parsed
        .upstreams
        .iter()
        .cloned()
        .map(Upstream::from_recipe)
        .collect::<Result<Vec<_>, _>>()?;

    println!();
    println!("Sharing {} upstream(s) with the build container", upstreams.len());
    println!();

    let mp = MultiProgress::new();
    let tp = mp.add(
        ProgressBar::new(upstreams.len() as u64).with_style(
            ProgressStyle::with_template("\n|{bar:20.cyan/blue}| {pos}/{len}")
                .unwrap()
                .progress_chars("■≡=- "),
        ),
    );
    tp.tick();

    let upstream_dir = paths.guest_host_path(&paths.upstreams());
    util::ensure_dir_exists(&upstream_dir).await?;

    stream::iter(&upstreams)
        .map(|upstream| async {
            let pb = mp.insert_before(
                &tp,
                ProgressBar::new(u64::MAX)
                    .with_message(format!("{} {}", "Downloading".blue(), upstream.name().bold(),)),
            );
            pb.enable_steady_tick(Duration::from_millis(150));

            let install = upstream.fetch(paths, &pb).await?;

            pb.set_message(format!("{} {}", "Copying".yellow(), upstream.name().bold(),));
            pb.set_style(
                ProgressStyle::with_template(" {spinner} {wide_msg} ")
                    .unwrap()
                    .tick_chars("--=≡■≡=--"),
            );

            install.share(&upstream_dir).await?;

            let cached_tag = install
                .was_cached()
                .then_some(format!("{}", " (cached)".dim()))
                .unwrap_or_default();

            pb.finish();
            mp.remove(&pb);
            mp.println(format!("{} {}{}", "Shared".green(), upstream.name().bold(), cached_tag,))?;
            tp.inc(1);

            Ok(()) as Result<_, Error>
        })
        .buffer_unordered(moss::environment::MAX_NETWORK_CONCURRENCY)
        .try_collect::<()>()
        .await?;

    mp.clear()?;
    println!();

    Ok(())
}

enum Installed {
    Plain {
        name: String,
        path: PathBuf,
        was_cached: bool,
    },
    Git {
        name: String,
        path: PathBuf,
        was_cached: bool,
    },
}

impl Installed {
    fn was_cached(&self) -> bool {
        match self {
            Installed::Plain { was_cached, .. } => *was_cached,
            Installed::Git { was_cached, .. } => *was_cached,
        }
    }

    async fn share(&self, dest_dir: &Path) -> Result<(), Error> {
        match self {
            Installed::Plain { name, path, .. } => {
                let target = dest_dir.join(name);

                // Attempt hard link
                let link_result = linkat(None, path, None, &target, LinkatFlags::NoSymlinkFollow);

                // Copy instead
                if link_result.is_err() {
                    copy(&path, &target).await?;
                }
            }
            Installed::Git { name, path, .. } => {
                let target = dest_dir.join(name);
                util::copy_dir(path, &target).await?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Upstream {
    Plain(Plain),
    Git(Git),
}

impl Upstream {
    pub fn from_recipe(upstream: stone_recipe::Upstream) -> Result<Self, Error> {
        match upstream {
            stone_recipe::Upstream::Plain { uri, hash, rename, .. } => Ok(Self::Plain(Plain {
                uri,
                hash: hash.parse()?,
                rename,
            })),
            stone_recipe::Upstream::Git {
                uri, ref_id, staging, ..
            } => Ok(Self::Git(Git { uri, ref_id, staging })),
        }
    }

    fn name(&self) -> &str {
        match self {
            Upstream::Plain(plain) => plain.name(),
            Upstream::Git(git) => git.name(),
        }
    }

    async fn fetch(&self, paths: &Paths, pb: &ProgressBar) -> Result<Installed, Error> {
        match self {
            Upstream::Plain(plain) => plain.fetch(paths, pb).await,
            Upstream::Git(git) => git.fetch(paths, pb).await,
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
}

impl Plain {
    fn name(&self) -> &str {
        if let Some(name) = &self.rename {
            name
        } else {
            util::uri_file_name(&self.uri)
        }
    }

    async fn path(&self, paths: &Paths) -> PathBuf {
        // Type safe guaranteed to be >= 5 bytes
        let hash = &self.hash.0;

        let parent = paths
            .upstreams()
            .host
            .join("fetched")
            .join(&hash[..5])
            .join(&hash[hash.len() - 5..]);

        let _ = util::ensure_dir_exists(&parent).await;

        parent.join(hash)
    }

    async fn fetch(&self, paths: &Paths, pb: &ProgressBar) -> Result<Installed, Error> {
        use moss::request;
        use tokio::fs;

        pb.set_style(
            ProgressStyle::with_template(" {spinner} {wide_msg} {binary_bytes_per_sec:>.dim} ")
                .unwrap()
                .tick_chars("--=≡■≡=--"),
        );

        let name = self.name();
        let path = self.path(paths).await;

        if path.exists() {
            return Ok(Installed::Plain {
                name: name.to_string(),
                path,
                was_cached: true,
            });
        }

        let mut stream = request::get(self.uri.clone()).await?;

        let mut hasher = Sha256::new();
        let mut out = fs::File::create(&path).await?;

        while let Some(chunk) = stream.next().await {
            let bytes = &chunk?;
            pb.inc(bytes.len() as u64);
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

        Ok(Installed::Plain {
            name: name.to_string(),
            path,
            was_cached: false,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Git {
    uri: Url,
    ref_id: String,
    staging: bool,
}

impl Git {
    fn name(&self) -> &str {
        util::uri_file_name(&self.uri)
    }

    async fn final_path(&self, paths: &Paths) -> PathBuf {
        let parent = paths.upstreams().host.join("git");

        let _ = util::ensure_dir_exists(&parent).await;

        parent.join(util::uri_relative_path(&self.uri))
    }

    async fn staging_path(&self, paths: &Paths) -> PathBuf {
        let parent = paths.upstreams().host.join("staging").join("git");

        let _ = util::ensure_dir_exists(&parent).await;

        parent.join(util::uri_relative_path(&self.uri))
    }

    async fn fetch(&self, paths: &Paths, pb: &ProgressBar) -> Result<Installed, Error> {
        pb.set_style(
            ProgressStyle::with_template(" {spinner} {wide_msg} ")
                .unwrap()
                .tick_chars("--=≡■≡=--"),
        );

        let clone_path = if self.staging {
            self.staging_path(paths).await
        } else {
            self.final_path(paths).await
        };
        let clone_path_string = clone_path.display().to_string();

        let final_path = self.final_path(paths).await;
        let final_path_string = final_path.display().to_string();

        if self.ref_exists(&final_path).await? {
            self.reset_to_ref(&final_path).await?;
            return Ok(Installed::Git {
                name: self.name().to_string(),
                path: final_path,
                was_cached: true,
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
            self.run(&["clone", "--", &clone_path_string, &final_path_string], None)
                .await?;
        }

        self.reset_to_ref(&final_path).await?;

        Ok(Installed::Git {
            name: self.name().to_string(),
            path: final_path,
            was_cached: false,
        })
    }

    async fn ref_exists(&self, path: &Path) -> Result<bool, Error> {
        if !path.exists() {
            return Ok(false);
        }

        self.run(&["fetch"], Some(path)).await?;

        let result = self.run(&["cat-file", "-e", &self.ref_id], Some(path)).await;

        Ok(result.is_ok())
    }

    async fn reset_to_ref(&self, path: &Path) -> Result<(), Error> {
        self.run(&["reset", "--hard", &self.ref_id], Some(path)).await?;

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
