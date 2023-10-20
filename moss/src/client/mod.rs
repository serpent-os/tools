// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{io, path::PathBuf, time::Duration};

use futures::{future::try_join_all, stream, StreamExt, TryStreamExt};
use itertools::Itertools;
use stone::read::Payload;
use thiserror::Error;
use tokio::fs;
use tui::{MultiProgress, ProgressBar, ProgressStyle, Stylize};

use self::prune::prune;
use crate::{
    db,
    package::{self, cache},
    registry::plugin::{self, Plugin},
    repository, Installation, Package, Registry, State,
};

pub mod prune;

const CONCURRENT_TASKS: usize = 8;

#[derive(Debug, Error)]
pub enum Error {
    #[error("corrupted package")]
    CorruptedPackage,
    #[error("No metadata found for package {0:?}")]
    MissingMetadata(package::Id),
    #[error("Root is invalid")]
    RootInvalid,
    #[error("package cache error: {0}")]
    Cache(#[from] package::cache::Error),
    #[error("repository: {0}")]
    Repository(#[from] repository::manager::Error),
    #[error("meta: {0}")]
    Meta(#[from] db::meta::Error),
    #[error("layout: {0}")]
    Layout(#[from] db::layout::Error),
    #[error("state: {0}")]
    State(#[from] db::state::Error),
    #[error("prune: {0}")]
    Prune(#[from] prune::Error),
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

/// A Client is a connection to the underlying package management systems
pub struct Client {
    /// Root that we operate on
    pub installation: Installation,
    repositories: repository::Manager,
    pub registry: Registry,

    pub install_db: db::meta::Database,
    pub state_db: db::state::Database,
    pub layout_db: db::layout::Database,
}

impl Client {
    /// Construct a new Client
    pub async fn new_for_root(root: impl Into<PathBuf>) -> Result<Client, Error> {
        let root = root.into();

        if !root.exists() || !root.is_dir() {
            return Err(Error::RootInvalid);
        }

        let installation = Installation::open(root);
        let repositories = repository::Manager::new(installation.clone()).await?;
        let install_db =
            db::meta::Database::new(installation.db_path("install"), installation.read_only())
                .await?;
        let state_db = db::state::Database::new(&installation).await?;
        let layout_db = db::layout::Database::new(&installation).await?;

        let state = match installation.active_state {
            Some(id) => Some(state_db.get(&id).await?),
            None => None,
        };

        let registry = build_registry(&repositories, &install_db, state).await?;

        Ok(Client {
            installation,
            repositories,
            registry,
            install_db,
            state_db,
            layout_db,
        })
    }

    /// Construct a new Client for the global installation
    pub async fn system() -> Result<Client, Error> {
        Client::new_for_root("/").await
    }

    /// Reload all configured repositories and refreshes their index file, then update
    /// registry with all active repositories.
    pub async fn refresh_repositories(&mut self) -> Result<(), Error> {
        // Reload manager and refresh all repositories
        self.repositories = repository::Manager::new(self.installation.clone()).await?;
        self.repositories.refresh_all().await?;

        // Refresh State DB
        let state = match self.installation.active_state {
            Some(id) => Some(self.state_db.get(&id).await?),
            None => None,
        };

        // Rebuild registry
        self.registry = build_registry(&self.repositories, &self.install_db, state).await?;

        Ok(())
    }

    pub async fn prune(&self, strategy: prune::Strategy) -> Result<(), Error> {
        prune(
            strategy,
            &self.state_db,
            &self.install_db,
            &self.layout_db,
            &self.installation,
        )
        .await?;
        Ok(())
    }

    /// Fetches package metadata for the provided packages
    /// from the underlying registry. Returned metadata is
    /// deduped & sorted by package name.
    pub async fn get_metadata(
        &self,
        packages: impl IntoIterator<Item = &package::Id>,
    ) -> Result<Vec<Package>, Error> {
        let mut metadata = try_join_all(packages.into_iter().map(|id| async {
            self.registry
                .by_id(id)
                .boxed()
                .next()
                .await
                .ok_or(Error::MissingMetadata(id.clone()))
        }))
        .await?;
        metadata.sort_by_key(|p| p.meta.name.to_string());
        metadata.dedup_by_key(|p| p.meta.name.to_string());
        Ok(metadata)
    }

    /// Create a new recorded state from the provided packages
    /// provided packages and write that state ID to the installation
    pub async fn record_state(
        &self,
        packages: &[package::Id],
        summary: impl ToString,
    ) -> Result<State, Error> {
        // Add to db
        let state = self
            .state_db
            .add(packages, Some(summary.to_string()), None)
            .await?;

        // Write state id
        {
            let usr = self.installation.root.join("usr");
            fs::create_dir_all(&usr).await?;
            let state_path = usr.join(".stateID");
            fs::write(state_path, state.id.to_string()).await?;
        }

        Ok(state)
    }

    /// Download & unpack the provided packages
    pub async fn cache_packages(&self, packages: &[&Package]) -> Result<(), Error> {
        // Setup progress bar
        let multi_progress = MultiProgress::new();

        // Add bar to track total package counts
        let total_progress = multi_progress.add(
            ProgressBar::new(packages.len() as u64).with_style(
                ProgressStyle::with_template("\n|{bar:20.cyan/blue}| {pos}/{len}")
                    .unwrap()
                    .progress_chars("■≡=- "),
            ),
        );
        total_progress.tick();

        // Download and unpack each package
        stream::iter(packages.iter().map(|package| async {
            // Setup the progress bar and set as downloading
            let progress_bar = multi_progress.insert_before(
                &total_progress,
                ProgressBar::new(package.meta.download_size.unwrap_or_default())
                    .with_message(format!(
                        "{} {}",
                        "Downloading".blue(),
                        package.meta.name.to_string().bold(),
                    ))
                    .with_style(
                        ProgressStyle::with_template(
                            " {spinner} |{percent:>3}%| {wide_msg} {binary_bytes_per_sec:>.dim} ",
                        )
                        .unwrap()
                        .tick_chars("--=≡■≡=--"),
                    ),
            );
            progress_bar.enable_steady_tick(Duration::from_millis(150));

            // Download and update progress
            let download = cache::fetch(&package.meta, &self.installation, |progress| {
                progress_bar.inc(progress.delta);
            })
            .await?;

            let is_cached = download.was_cached;
            let package_name = package.meta.name.to_string();

            // Set progress to unpacking
            progress_bar.set_message(format!(
                "{} {}",
                "Unpacking".yellow(),
                package_name.clone().bold(),
            ));
            progress_bar.set_length(1000);
            progress_bar.set_position(0);

            // Unpack and update progress
            let unpacked = download
                .unpack({
                    let progress_bar = progress_bar.clone();

                    move |progress| {
                        progress_bar.set_position((progress.pct() * 1000.0) as u64);
                    }
                })
                .await?;

            // Merge layoutdb
            progress_bar.set_message(format!(
                "{} {}",
                "Store layout".white(),
                package_name.clone().bold()
            ));
            // Remove old layout entries for package
            self.layout_db.remove(&package.id).await?;
            // Add new entries in batches of 1k
            for chunk in progress_bar.wrap_iter(
                unpacked
                    .payloads
                    .iter()
                    .find_map(Payload::layout)
                    .ok_or(Error::CorruptedPackage)?
                    .chunks(1000),
            ) {
                let entries = chunk
                    .iter()
                    .map(|i| (package.id.clone(), i.clone()))
                    .collect_vec();
                self.layout_db.batch_add(entries).await?;
            }

            // Consume the package in the metadb
            self.install_db
                .add(package.id.clone(), package.meta.clone())
                .await?;

            // Remove this progress bar
            progress_bar.finish();
            multi_progress.remove(&progress_bar);

            let cached_tag = is_cached
                .then_some(format!("{}", " (cached)".dim()))
                .unwrap_or_default();

            // Write installed line
            multi_progress.println(format!(
                "{} {}{}",
                "Installed".green(),
                package_name.clone().bold(),
                cached_tag,
            ))?;

            // Inc total progress by 1
            total_progress.inc(1);

            Ok(()) as Result<(), Error>
        }))
        .buffer_unordered(CONCURRENT_TASKS)
        .try_collect()
        .await?;

        // Remove progress
        multi_progress.clear()?;

        Ok(())
    }
}

async fn build_registry(
    repositories: &repository::Manager,
    installdb: &db::meta::Database,
    state: Option<State>,
) -> Result<Registry, Error> {
    let mut registry = Registry::default();

    registry.add_plugin(Plugin::Cobble(plugin::Cobble::default()));
    registry.add_plugin(Plugin::Active(plugin::Active::new(
        state,
        installdb.clone(),
    )));

    for repo in repositories.active() {
        registry.add_plugin(Plugin::Repository(plugin::Repository::new(repo)));
    }

    Ok(registry)
}

impl Drop for Client {
    // Automatically drop resources for the client
    fn drop(&mut self) {}
}
