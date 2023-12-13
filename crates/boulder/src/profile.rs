// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{collections::HashMap, fmt};

use config::Config;
use moss::repository;
pub use moss::{repository::Priority, Repository};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Env;

/// A unique [`Profile`] identifier
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

impl From<String> for Id {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Profile configuration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub collections: repository::Map,
}

/// A map of profiles
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Map(HashMap<Id, Profile>);

impl Map {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn with(items: impl IntoIterator<Item = (Id, Profile)>) -> Self {
        Self(items.into_iter().collect())
    }

    pub fn get(&self, id: &Id) -> Option<&Profile> {
        self.0.get(id)
    }

    pub fn add(&mut self, id: Id, profile: Profile) {
        self.0.insert(id, profile);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Id, &Profile)> {
        self.0.iter()
    }
}

impl IntoIterator for Map {
    type Item = (Id, Profile);
    type IntoIter = std::collections::hash_map::IntoIter<Id, Profile>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Config for Map {
    fn domain() -> String {
        "profile".into()
    }

    fn merge(self, other: Self) -> Self {
        Self(self.0.into_iter().chain(other.0).collect())
    }
}

pub struct Manager<'a> {
    pub profiles: Map,
    env: &'a Env,
}

impl<'a> Manager<'a> {
    pub async fn new(env: &'a Env) -> Manager<'a> {
        let profiles = env.config.load::<Map>().await.unwrap_or_default();

        Self { env, profiles }
    }

    pub fn repositories(&self, profile: &Id) -> Result<&repository::Map, Error> {
        self.profiles
            .get(profile)
            .map(|profile| &profile.collections)
            .ok_or(Error::MissingProfile)
    }

    pub async fn save_profile(&mut self, id: Id, profile: Profile) -> Result<(), Error> {
        // Save config
        let map = Map::with([(id.clone(), profile.clone())]);
        self.env.config.save(id.clone(), &map).await?;

        // Add to profile map
        self.profiles.add(id, profile);

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot find the provided profile")]
    MissingProfile,
    #[error("save profiles")]
    SaveProfile(#[from] config::SaveError),
}
