// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    path::PathBuf,
};

use moss::{stone::write::digest, Dependency, Provider};
use tui::Stylize;

use super::collect::{Collector, PathInfo};
use crate::{Paths, Recipe};

mod handler;

pub type BoxError = Box<dyn std::error::Error>;

pub struct Chain<'a> {
    handlers: Vec<Box<dyn Handler>>,
    recipe: &'a Recipe,
    paths: &'a Paths,
    collector: &'a Collector,
    hasher: &'a mut digest::Hasher,
    pub buckets: HashMap<String, Bucket>,
}

impl<'a> Chain<'a> {
    pub fn new(paths: &'a Paths, recipe: &'a Recipe, collector: &'a Collector, hasher: &'a mut digest::Hasher) -> Self {
        Self {
            handlers: vec![
                Box::new(handler::ignore_blocked),
                Box::new(handler::binary),
                Box::new(handler::elf),
                Box::new(handler::pkg_config),
                Box::new(handler::cmake),
                // Catch-all if not excluded
                Box::new(handler::include_any),
            ],
            paths,
            recipe,
            collector,
            hasher,
            buckets: Default::default(),
        }
    }

    pub fn process(&mut self, paths: impl IntoIterator<Item = PathInfo>) -> Result<(), BoxError> {
        let mut queue = paths.into_iter().collect::<VecDeque<_>>();

        'paths: while let Some(mut path) = queue.pop_front() {
            let bucket = self.buckets.entry(path.package.clone()).or_default();

            'handlers: for handler in &self.handlers {
                // Only give handlers ability to update
                // certain bucket fields
                let mut bucket_mut = BucketMut {
                    providers: &mut bucket.providers,
                    dependencies: &mut bucket.dependencies,
                    hasher: self.hasher,
                    recipe: self.recipe,
                    paths: self.paths,
                };

                let response = handler.handle(&mut bucket_mut, &mut path)?;

                response.generated_paths.into_iter().try_for_each(|path| {
                    let info = self.collector.path(&path, self.hasher)?;

                    queue.push_back(info);

                    Ok(()) as Result<(), BoxError>
                })?;

                match response.decision {
                    Decision::NextHandler => continue 'handlers,
                    Decision::IgnoreFile { reason } => {
                        // TODO: Proper logging so we can log from various places
                        // and have consistent output
                        eprintln!(
                            "[analysis] {} - {reason}, ignoring {}",
                            "WARN".yellow(),
                            path.target_path.display()
                        );
                        continue 'paths;
                    }
                    Decision::IncludeFile => {
                        bucket.paths.push(path);
                        continue 'paths;
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Bucket {
    pub providers: BTreeSet<Provider>,
    pub dependencies: BTreeSet<Dependency>,
    pub paths: Vec<PathInfo>,
}

pub struct BucketMut<'a> {
    pub providers: &'a mut BTreeSet<Provider>,
    pub dependencies: &'a mut BTreeSet<Dependency>,
    pub hasher: &'a mut digest::Hasher,
    pub recipe: &'a Recipe,
    pub paths: &'a Paths,
}

pub struct Response {
    pub decision: Decision,
    pub generated_paths: Vec<PathBuf>,
}

pub enum Decision {
    NextHandler,
    IgnoreFile { reason: String },
    IncludeFile,
}

impl From<Decision> for Response {
    fn from(decision: Decision) -> Self {
        Self {
            decision,
            generated_paths: vec![],
        }
    }
}

pub trait Handler {
    fn handle(&self, bucket: &mut BucketMut<'_>, path: &mut PathInfo) -> Result<Response, BoxError>;
}

impl<T> Handler for T
where
    T: Fn(&mut BucketMut<'_>, &mut PathInfo) -> Result<Response, BoxError>,
{
    fn handle(&self, bucket: &mut BucketMut<'_>, path: &mut PathInfo) -> Result<Response, BoxError> {
        (self)(bucket, path)
    }
}
