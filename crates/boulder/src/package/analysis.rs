// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::{BTreeSet, HashMap, VecDeque};

use moss::{Dependency, Provider};
use tui::Stylize;

use super::collect::PathInfo;

mod handler;

pub type BoxError = Box<dyn std::error::Error>;

pub struct Chain {
    handlers: Vec<Box<dyn Handler>>,
    pub buckets: HashMap<String, Bucket>,
}

impl Chain {
    pub fn new() -> Self {
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
                };

                let response = handler.handle(&mut bucket_mut, &mut path)?;

                response
                    .generated_paths
                    .into_iter()
                    .for_each(|path| queue.push_back(path));

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

#[derive(Debug)]
pub struct BucketMut<'a> {
    pub providers: &'a mut BTreeSet<Provider>,
    pub dependencies: &'a mut BTreeSet<Dependency>,
}

pub struct Response {
    pub decision: Decision,
    pub generated_paths: Vec<PathInfo>,
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
