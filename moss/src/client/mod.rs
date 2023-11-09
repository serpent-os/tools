// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    io,
    os::fd::RawFd,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::{future::try_join_all, stream, StreamExt, TryStreamExt};
use itertools::Itertools;
use nix::{
    errno::Errno,
    fcntl::{self, renameat2, OFlag, RenameFlags},
    sys::stat::{fchmodat, mkdirat, Mode},
    unistd::{close, linkat, mkdir, symlinkat},
};
use stone::{payload::layout, read::Payload};
use thiserror::Error;
use tokio::fs::{self, create_dir_all, remove_dir_all, remove_file, rename, symlink};
use tui::{MultiProgress, ProgressBar, ProgressStyle, Stylize};
use vfs::tree::{builder::TreeBuilder, BlitFile, Element};

use self::prune::prune;
use crate::{
    db, environment, package,
    registry::plugin::{self, Plugin},
    repository,
    state::{self, Selection},
    Installation, Package, Registry, State,
};

pub mod cache;
pub mod prune;

/// A Client is a connection to the underlying package management systems
pub struct Client {
    /// Root that we operate on
    pub installation: Installation,
    pub registry: Registry,

    pub install_db: db::meta::Database,
    pub state_db: db::state::Database,
    pub layout_db: db::layout::Database,

    repositories: repository::Manager,
    scope: Scope,
}

impl Client {
    /// Construct a new Client
    pub async fn new(root: impl Into<PathBuf>) -> Result<Client, Error> {
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
            scope: Scope::Stateful,
        })
    }

    /// Construct a new Client for the global installation
    pub async fn system() -> Result<Client, Error> {
        Client::new("/").await
    }

    pub fn is_ephemeral(&self) -> bool {
        matches!(self.scope, Scope::Ephemeral { .. })
    }

    /// Transition to an ephemeral client that doesn't record state changes
    /// and blits to a different root.
    ///
    /// This is useful for installing a root to a container (i.e. Boulder) while
    /// using a shared cache.
    ///
    /// Returns an error if `blit_root` is the same as the installation root,
    /// since the system client should always be stateful.
    pub fn ephemeral(self, blit_root: PathBuf) -> Result<Self, Error> {
        if blit_root.canonicalize()? == self.installation.root.canonicalize()? {
            return Err(Error::EphemeralInstallationRoot);
        }

        Ok(Self {
            scope: Scope::Ephemeral { blit_root },
            ..self
        })
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

    /// Prune states with the provided [`prune::Strategy`]
    pub async fn prune(&self, strategy: prune::Strategy) -> Result<(), Error> {
        if self.scope.is_ephemeral() {
            return Err(Error::EphemeralProhibitedOperation);
        }

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

    /// Resolves the provided id's with the underlying registry, returning
    /// the first [`Package`] for each id. Packages are sorted by name
    /// and deduped before returning.
    pub async fn resolve_packages(
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
    /// Then blit the filesystem, promote it, finally archiving the active ID
    ///
    /// Returns `None` if the client is ephemeral
    pub async fn apply_state(
        &self,
        selections: &[Selection],
        summary: impl ToString,
    ) -> Result<Option<State>, Error> {
        let old_state = self.installation.active_state;

        self.blit_root(
            selections.iter().map(|s| &s.package),
            old_state.map(state::Id::next),
        )
        .await?;

        if !self.scope.is_ephemeral() {
            // Add to db
            let state = self
                .state_db
                .add(selections, Some(summary.to_string()), None)
                .await?;

            // Write state id
            {
                let usr = self.installation.staging_path("usr");
                fs::create_dir_all(&usr).await?;
                let state_path = usr.join(".stateID");
                fs::write(state_path, state.id.to_string()).await?;
            }

            // Staging is only used with [`Scope::Stateful`]
            self.promote_staging().await?;

            if let Some(id) = old_state {
                self.archive_state(id).await?;
            }

            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    /// Activate the given state
    async fn promote_staging(&self) -> Result<(), Error> {
        if self.scope.is_ephemeral() {
            return Err(Error::EphemeralProhibitedOperation);
        }

        let usr_target = self.installation.root.join("usr");
        let usr_source = self.installation.staging_path("usr");

        // Create the target tree
        if !usr_target.try_exists()? {
            create_dir_all(&usr_target).await?;
        }

        // Now swap staging with live
        renameat2(
            None,
            &usr_source,
            None,
            &usr_target,
            RenameFlags::RENAME_EXCHANGE,
        )?;

        Ok(())
    }

    /// Archive old states into their respective tree
    async fn archive_state(&self, id: state::Id) -> Result<(), Error> {
        if self.scope.is_ephemeral() {
            return Err(Error::EphemeralProhibitedOperation);
        }

        // After promotion, the old active /usr is now in staging/usr
        let usr_target = self.installation.root_path(id.to_string()).join("usr");
        let usr_source = self.installation.staging_path("usr");
        if let Some(parent) = usr_target.parent() {
            if !parent.exists() {
                create_dir_all(parent).await?;
            }
        }
        // hot swap the staging/usr into the root/$id/usr
        rename(&usr_source, &usr_target).await?;
        Ok(())
    }

    /// Download & unpack the provided packages. Packages already cached will be validated & skipped.
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
                    .chunks(environment::DB_BATCH_SIZE),
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
        // Use max network concurrency since we download files here
        .buffer_unordered(environment::MAX_NETWORK_CONCURRENCY)
        .try_collect()
        .await?;

        // Remove progress
        multi_progress.clear()?;

        Ok(())
    }

    /// Blit the packages to a filesystem root
    async fn blit_root(
        &self,
        packages: impl IntoIterator<Item = &package::Id>,
        state_id: Option<state::Id>,
    ) -> Result<(), Error> {
        let progress = ProgressBar::new(1).with_style(
            ProgressStyle::with_template("\n|{bar:20.red/blue}| {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("■≡=- "),
        );
        progress.set_message("Blitting filesystem");
        progress.enable_steady_tick(Duration::from_millis(150));
        progress.tick();

        let mut tbuild = TreeBuilder::new();
        for id in packages.into_iter() {
            let layouts = self.layout_db.query(id).await?;
            for layout in layouts {
                tbuild.push(PendingFile {
                    id: id.clone(),
                    layout,
                });
            }
        }
        tbuild.bake();
        let tree = tbuild.tree()?;
        progress.set_length(tree.len());
        progress.set_position(0_u64);

        let cache_dir = self.installation.assets_path("v2");
        let cache_fd = fcntl::open(
            &cache_dir,
            OFlag::O_DIRECTORY | OFlag::O_RDONLY,
            Mode::empty(),
        )?;

        let blit_target = match &self.scope {
            Scope::Stateful => self.installation.staging_dir(),
            Scope::Ephemeral { blit_root } => blit_root.to_owned(),
        };

        // undirt.
        remove_dir_all(&blit_target).await?;

        if let Some(root) = tree.structured() {
            let _ = mkdir(&blit_target, Mode::from_bits_truncate(0o755));
            let root_dir = fcntl::open(
                &blit_target,
                OFlag::O_DIRECTORY | OFlag::O_RDONLY,
                Mode::empty(),
            )?;

            if let Element::Directory(_, _, children) = root {
                for child in children {
                    self.blit_element(root_dir, cache_fd, child, &progress)?;
                }
            }

            close(root_dir)?;
        }

        finalize_root(&blit_target, state_id).await?;

        Ok(())
    }

    /// blit an element to the disk.
    fn blit_element(
        &self,
        parent: RawFd,
        cache: RawFd,
        element: Element<PendingFile>,
        progress: &ProgressBar,
    ) -> Result<(), Error> {
        progress.inc(1);
        match element {
            Element::Directory(name, item, children) => {
                // Construct within the parent
                self.blit_element_item(parent, cache, &name, item)?;

                // open the new dir
                let newdir = fcntl::openat(
                    parent,
                    name.as_str(),
                    OFlag::O_RDONLY | OFlag::O_DIRECTORY,
                    Mode::empty(),
                )?;
                for child in children.into_iter() {
                    self.blit_element(newdir, cache, child, progress)?;
                }
                close(newdir)?;
                Ok(())
            }
            Element::Child(name, item) => {
                self.blit_element_item(parent, cache, &name, item)?;
                Ok(())
            }
        }
    }

    /// Process the raw layout entry.
    fn blit_element_item(
        &self,
        parent: RawFd,
        cache: RawFd,
        subpath: &str,
        item: PendingFile,
    ) -> Result<(), Error> {
        match item.layout.entry {
            layout::Entry::Regular(id, _) => {
                let hash = format!("{:02x}", id);
                let directory = if hash.len() >= 10 {
                    PathBuf::from(&hash[..2])
                        .join(&hash[2..4])
                        .join(&hash[4..6])
                } else {
                    "".into()
                };

                // Link relative from cache to target
                let fp = directory.join(hash);
                linkat(
                    Some(cache),
                    fp.to_str().unwrap(),
                    Some(parent),
                    subpath,
                    nix::unistd::LinkatFlags::NoSymlinkFollow,
                )?;

                // Fix permissions
                fchmodat(
                    Some(parent),
                    subpath,
                    Mode::from_bits_truncate(item.layout.mode),
                    nix::sys::stat::FchmodatFlags::NoFollowSymlink,
                )?;
            }
            layout::Entry::Symlink(source, _) => {
                symlinkat(source.as_str(), Some(parent), subpath)?;
            }
            layout::Entry::Directory(_) => {
                mkdirat(parent, subpath, Mode::from_bits_truncate(item.layout.mode))?;
            }

            // unimplemented
            layout::Entry::CharacterDevice(_) => todo!(),
            layout::Entry::BlockDevice(_) => todo!(),
            layout::Entry::Fifo(_) => todo!(),
            layout::Entry::Socket(_) => todo!(),
        };

        Ok(())
    }
}

/// Add root symlinks & os-release file
async fn finalize_root(root: &Path, state_id: Option<state::Id>) -> Result<(), Error> {
    let links = vec![
        ("usr/sbin", "sbin"),
        ("usr/bin", "bin"),
        ("usr/lib", "lib"),
        ("usr/lib", "lib64"),
        ("usr/lib32", "lib32"),
    ];

    'linker: for (source, target) in links.into_iter() {
        let final_target = root.join(target);
        let staging_target = root.join(format!("{target}.next"));

        if staging_target.exists() {
            remove_file(&staging_target).await?;
        }

        if final_target.exists()
            && final_target.is_symlink()
            && final_target.read_link()?.to_string_lossy() == source
        {
            continue 'linker;
        }
        symlink(source, &staging_target).await?;
        rename(staging_target, final_target).await?;
    }

    let os_release = format!(
        r#"NAME="Serpent OS"
VERSION="{version}"
ID="serpentos"
VERSION_CODENAME={version}
VERSION_ID="{version}"
PRETTY_NAME="Serpent OS {version} (fstx #{tx})"
ANSI_COLOR="1;35"
HOME_URL="https://serpentos.com"
BUG_REPORT_URL="https://github.com/serpent-os""#,
        version = environment::VERSION,
        // TODO: Better id for ephemeral transactions
        tx = state_id.unwrap_or_default()
    );

    fs::write(root.join("usr").join("lib").join("os-release"), os_release).await?;

    Ok(())
}

enum Scope {
    Stateful,
    Ephemeral { blit_root: PathBuf },
}

impl Scope {
    fn is_ephemeral(&self) -> bool {
        matches!(self, Self::Ephemeral { .. })
    }
}

/// A pending file for blitting
#[derive(Debug, Clone)]
struct PendingFile {
    id: package::Id,
    layout: layout::Layout,
}

impl BlitFile for PendingFile {
    /// Match internal kind to minimalist vfs kind
    fn kind(&self) -> vfs::tree::Kind {
        match &self.layout.entry {
            layout::Entry::Symlink(source, _) => vfs::tree::Kind::Symlink(source.clone()),
            layout::Entry::Directory(_) => vfs::tree::Kind::Directory,
            _ => vfs::tree::Kind::Regular,
        }
    }

    /// Return ID for conflict
    fn id(&self) -> String {
        self.id.clone().into()
    }

    /// Resolve the target path, including the missing `/usr` prefix
    fn path(&self) -> PathBuf {
        let result = match &self.layout.entry {
            layout::Entry::Regular(_, target) => target.clone(),
            layout::Entry::Symlink(_, target) => target.clone(),
            layout::Entry::Directory(target) => target.clone(),
            layout::Entry::CharacterDevice(target) => target.clone(),
            layout::Entry::BlockDevice(target) => target.clone(),
            layout::Entry::Fifo(target) => target.clone(),
            layout::Entry::Socket(target) => target.clone(),
        };
        PathBuf::from("/usr").join(result)
    }

    /// Clone the node to a reparented path, for symlink resolution
    fn cloned_to(&self, path: PathBuf) -> Self {
        let mut new = self.clone();
        let strpath = path.to_string_lossy().to_string();
        new.layout.entry = match &self.layout.entry {
            layout::Entry::Regular(source, _) => layout::Entry::Regular(*source, strpath),
            layout::Entry::Symlink(source, _) => layout::Entry::Symlink(source.clone(), strpath),
            layout::Entry::Directory(_) => layout::Entry::Directory(strpath),
            layout::Entry::CharacterDevice(_) => layout::Entry::CharacterDevice(strpath),
            layout::Entry::BlockDevice(_) => layout::Entry::BlockDevice(strpath),
            layout::Entry::Fifo(_) => layout::Entry::Fifo(strpath),
            layout::Entry::Socket(_) => layout::Entry::Socket(strpath),
        };
        new
    }
}

impl From<PathBuf> for PendingFile {
    fn from(value: PathBuf) -> Self {
        PendingFile {
            id: Default::default(),
            layout: layout::Layout {
                uid: 0,
                gid: 0,
                mode: 0o755,
                tag: 0,
                entry: layout::Entry::Directory(value.to_string_lossy().to_string()),
            },
        }
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("Corrupted package")]
    CorruptedPackage,
    #[error("No metadata found for package {0:?}")]
    MissingMetadata(package::Id),
    #[error("Root is invalid")]
    RootInvalid,
    #[error("Ephemeral client not allowed on installation root")]
    EphemeralInstallationRoot,
    #[error("Operation not allowed with ephemeral client")]
    EphemeralProhibitedOperation,
    #[error("cache")]
    Cache(#[from] cache::Error),
    #[error("repository manager")]
    Repository(#[from] repository::manager::Error),
    #[error("meta db")]
    Meta(#[from] db::meta::Error),
    #[error("layout db")]
    Layout(#[from] db::layout::Error),
    #[error("state db")]
    State(#[from] db::state::Error),
    #[error("prune")]
    Prune(#[from] prune::Error),
    #[error("io")]
    Io(#[from] io::Error),
    #[error("filesystem")]
    Filesystem(#[from] vfs::tree::Error),
    #[error("blit")]
    Blit(#[from] Errno),
}
