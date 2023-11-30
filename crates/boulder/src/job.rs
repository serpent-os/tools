// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use stone_recipe::Recipe;
use thiserror::Error;

use crate::{util, Env};

#[derive(Debug, Clone)]
pub struct Id(String);

impl Id {
    fn new(recipe: &Recipe) -> Self {
        Self(format!(
            "{}-{}-{}",
            recipe.source.name, recipe.source.version, recipe.source.release
        ))
    }
}

pub struct Job {
    pub id: Id,
    pub recipe: Recipe,
    pub paths: Paths,
}

impl Job {
    pub async fn new(recipe_path: &Path, env: &Env) -> Result<Self, Error> {
        let recipe_bytes = fs::read(recipe_path)?;
        let recipe = stone_recipe::from_slice(&recipe_bytes)?;

        let id = Id::new(&recipe);

        let paths = Paths::new(id.clone(), recipe_path, &env.cache_dir, "/mason").await?;

        Ok(Self { id, recipe, paths })
    }
}

pub struct Paths {
    id: Id,
    host_root: PathBuf,
    guest_root: PathBuf,
    recipe_dir: PathBuf,
}

impl Paths {
    async fn new(
        id: Id,
        recipe_path: &Path,
        host_root: impl Into<PathBuf>,
        guest_root: impl Into<PathBuf>,
    ) -> io::Result<Self> {
        let recipe_dir = recipe_path
            .parent()
            .unwrap_or(&PathBuf::default())
            .canonicalize()?;

        let job = Self {
            id,
            host_root: host_root.into().canonicalize()?,
            guest_root: guest_root.into(),
            recipe_dir,
        };

        util::ensure_dir_exists(&job.rootfs().host).await?;
        util::ensure_dir_exists(&job.artefacts().host).await?;
        util::ensure_dir_exists(&job.build().host).await?;
        util::ensure_dir_exists(&job.ccache().host).await?;
        util::ensure_dir_exists(&job.upstreams().host).await?;

        Ok(job)
    }

    pub fn rootfs(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("root").join(&self.id.0),
            guest: "/".into(),
        }
    }

    pub fn artefacts(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("artefacts").join(&self.id.0),
            guest: self.guest_root.join("artefacts"),
        }
    }

    pub fn build(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("build").join(&self.id.0),
            guest: self.guest_root.join("build"),
        }
    }

    pub fn ccache(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("ccache"),
            guest: self.guest_root.join("ccache"),
        }
    }

    pub fn upstreams(&self) -> PathMapping {
        PathMapping {
            host: self.host_root.join("upstreams"),
            guest: self.guest_root.join("sourcedir"),
        }
    }

    pub fn recipe(&self) -> PathMapping {
        PathMapping {
            host: self.recipe_dir.clone(),
            guest: self.guest_root.join("recipe"),
        }
    }

    /// For the provided [`Mapping`], return the guest
    /// path as it lives on the host fs
    ///
    /// Example:
    /// - host = "/var/cache/boulder/root/test"
    /// - guest = "/mason/build"
    /// - guest_host_path = "/var/cache/boulder/root/test/mason/build"
    pub fn guest_host_path(&self, mapping: &PathMapping) -> PathBuf {
        let relative = mapping.guest.strip_prefix("/").unwrap_or(&mapping.guest);

        self.rootfs().host.join(relative)
    }
}

pub struct PathMapping {
    pub host: PathBuf,
    pub guest: PathBuf,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("stone recipe")]
    StoneRecipe(#[from] stone_recipe::Error),
    #[error("io")]
    Io(#[from] io::Error),
}
