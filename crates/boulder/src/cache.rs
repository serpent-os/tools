// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

use stone_recipe::Recipe;

struct Id(String);

impl Id {
    fn new(recipe: &Recipe) -> Self {
        Self(format!(
            "{}-{}-{}",
            recipe.source.name, recipe.source.version, recipe.source.release
        ))
    }
}

pub struct Cache {
    id: Id,
    host_root: PathBuf,
    guest_root: PathBuf,
}

impl Cache {
    pub fn new(
        recipe: &Recipe,
        host_root: impl Into<PathBuf>,
        guest_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id: Id::new(recipe),
            host_root: host_root.into(),
            guest_root: guest_root.into(),
        }
    }

    pub fn rootfs(&self) -> Mapping {
        Mapping {
            host: self.host_root.join("root").join(&self.id.0),
            guest: "/".into(),
        }
    }

    pub fn artefacts(&self) -> Mapping {
        Mapping {
            host: self.host_root.join("artefacts").join(&self.id.0),
            guest: self.guest_root.join("artefacts"),
        }
    }

    pub fn build(&self) -> Mapping {
        Mapping {
            host: self.host_root.join("build").join(&self.id.0),
            guest: self.guest_root.join("build"),
        }
    }

    pub fn ccache(&self) -> Mapping {
        Mapping {
            host: self.host_root.join("ccache"),
            guest: self.guest_root.join("ccache"),
        }
    }
}

pub struct Mapping {
    pub host: PathBuf,
    pub guest: PathBuf,
}
