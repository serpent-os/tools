// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Build a vfs tree incrementally
use std::{
    cmp,
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use crate::tree::{Kind, Tree};

use super::{BlitFile, Error};

/// Builder used to generate a full tree, free of conflicts
pub struct TreeBuilder<T: BlitFile> {
    entries: BTreeSet<Entry<T>>,
    symlinks: Vec<Symlink<T>>,
}

impl<T: BlitFile> Default for TreeBuilder<T> {
    fn default() -> Self {
        TreeBuilder {
            entries: BTreeSet::new(),
            symlinks: vec![],
        }
    }
}

impl<T: BlitFile> TreeBuilder<T> {
    pub fn new() -> Self {
        TreeBuilder::default()
    }

    /// Push an item to the builder
    pub fn push(&mut self, item: T) {
        // Add symlinks and return, we will process these
        // during `build`
        if let Some(resolved_source) = resolve_symlink_source(&item) {
            self.symlinks.push(Symlink {
                item,
                resolved_source,
            });
            return;
        }

        // Insert the provided entry
        self.entries.insert(Entry::new(item));
    }

    /// If a symlink directory (redirect) is encountered, all files under that direcory will
    /// get reparented to the redirect target directory.
    fn process_symlinks(&mut self) {
        let mut redirects = vec![];

        for Symlink {
            item,
            resolved_source,
        } in self.symlinks.drain(..)
        {
            let is_resolved_dir = self.entries.iter().any(|entry| {
                entry.inner().path() == resolved_source
                    && matches!(entry.inner().kind(), Kind::Directory)
            });

            // If this is a known directory, add it as the resolved directory
            if is_resolved_dir {
                redirects.push(Redirect {
                    from: item.path(),
                    to: resolved_source,
                });
            }
            // otherwise this is just a normal symlink so add it
            else {
                self.entries.insert(Entry::Other(item));
            }
        }

        // Process all redirects
        self.entries = std::mem::take(&mut self.entries)
            .into_iter()
            .map(|entry| redirect_entry(entry, &redirects))
            .collect();
    }

    /// Ensures all leading directories are added
    fn expand_directories(&mut self) {
        self.entries = std::mem::take(&mut self.entries)
            .into_iter()
            .flat_map(|entry| {
                enumerate_leading_dirs(&entry.inner().path())
                    .into_iter()
                    .map(|dir| Entry::Directory(dir.into()))
                    .chain(Some(entry))
            })
            .collect();
    }

    /// Build a [`Tree`] from the provided items
    pub fn build(mut self) -> Result<Tree<T>, Error> {
        self.process_symlinks();
        self.expand_directories();

        self.entries
            .into_iter()
            .try_fold(Tree::new(), |mut tree, entry| {
                let item = entry.into_inner();

                let path = item.path();
                let node = tree.new_node(item);

                if let Some(parent) = path.parent() {
                    tree.add_child_to_node(node, parent)?;
                }

                Ok(tree)
            })
    }
}

#[derive(Debug)]
enum Entry<T: BlitFile> {
    Directory(T),
    Other(T),
}

impl<T: BlitFile> Entry<T> {
    fn new(item: T) -> Self {
        match item.kind() {
            Kind::Directory => Entry::Directory(item),
            _ => Self::Other(item),
        }
    }

    fn inner(&self) -> &T {
        match self {
            Entry::Directory(inner) => inner,
            Entry::Other(inner) => inner,
        }
    }

    fn inner_mut(&mut self) -> &mut T {
        match self {
            Entry::Directory(inner) => inner,
            Entry::Other(inner) => inner,
        }
    }

    fn into_inner(self) -> T {
        match self {
            Entry::Directory(inner) => inner,
            Entry::Other(inner) => inner,
        }
    }
}

impl<T: BlitFile> PartialEq for Entry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner().path().eq(&other.inner().path())
    }
}

impl<T: BlitFile> Eq for Entry<T> {}

impl<T: BlitFile> PartialOrd for Entry<T> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// Order by directories first, then path ascending
impl<T: BlitFile> Ord for Entry<T> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match (self, other) {
            (Entry::Directory(_), Entry::Other(_)) => cmp::Ordering::Less,
            (Entry::Other(_), Entry::Directory(_)) => cmp::Ordering::Greater,
            (a, b) => a.inner().path().cmp(&b.inner().path()),
        }
    }
}

struct Symlink<T> {
    item: T,
    resolved_source: PathBuf,
}

fn resolve_symlink_source<T: BlitFile>(item: &T) -> Option<PathBuf> {
    let Kind::Symlink(source) = item.kind() else {
        return None;
    };

    let path = item.path();

    // Resolve the link.
    let source = if source.starts_with('/') {
        // Absolute
        source.into()
    } else if let Some(parent) = path.parent() {
        // Relative w/ parent
        parent.join(source)
    } else {
        // Relative to root
        source.into()
    };

    Some(normalize_path(source))
}

#[derive(Debug)]
struct Redirect {
    from: PathBuf,
    to: PathBuf,
}

/// Checks if entry falls under any redirects and redirects the entry if so. Otherwise,
/// returns the original entry.
fn redirect_entry<T: BlitFile>(mut entry: Entry<T>, redirects: &[Redirect]) -> Entry<T> {
    let path = entry.inner().path();

    let Some(redirected) = redirects.iter().find_map(|redirect| {
        path.strip_prefix(&redirect.from)
            .ok()
            .map(|relative| redirect.to.join(relative))
    }) else {
        return entry;
    };

    *entry.inner_mut() = entry.inner().cloned_to(redirected);

    entry
}

/// Remove `.` and `..` components
fn normalize_path(path: PathBuf) -> PathBuf {
    path.components()
        .fold(PathBuf::new(), |path, component| match component {
            std::path::Component::CurDir => path,
            // `parent` shouldn't fail here, but return non-nomalized otherwise
            std::path::Component::ParentDir => path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| path.join(component)),
            c => path.join(c),
        })
}

/// Returns all leading directories to the supplied path
fn enumerate_leading_dirs(path: &Path) -> Vec<PathBuf> {
    let Some(parent) = path.parent() else {
        return vec![];
    };

    parent
        .components()
        .scan(PathBuf::default(), |leading, component| {
            let path = leading.join(component);
            *leading = path.clone();
            Some(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::tree::Kind;

    #[derive(Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct CustomFile {
        path: PathBuf,
        kind: Kind,
    }

    impl From<PathBuf> for CustomFile {
        fn from(value: PathBuf) -> Self {
            Self {
                path: value,
                kind: Kind::Directory,
            }
        }
    }

    impl BlitFile for CustomFile {
        fn path(&self) -> PathBuf {
            self.path.clone()
        }

        fn kind(&self) -> Kind {
            self.kind.clone()
        }

        /// Clone to new path portion
        fn cloned_to(&self, path: PathBuf) -> Self {
            Self {
                path: path.clone(),
                kind: self.kind.clone(),
            }
        }
    }

    #[test]
    fn test_simple_root() {
        let mut b: TreeBuilder<CustomFile> = TreeBuilder::new();
        let paths = vec![
            CustomFile {
                path: "/usr/bin/nano".into(),
                kind: Kind::Regular,
            },
            CustomFile {
                path: "/usr/bin/rnano".into(),
                kind: Kind::Symlink("nano".to_string()),
            },
            CustomFile {
                path: "/usr/share/nano".into(),
                kind: Kind::Directory,
            },
            CustomFile {
                path: "/var/run/lock".into(),
                kind: Kind::Symlink("/run/lock".into()),
            },
            CustomFile {
                path: "/var/run/lock/subsys/1".into(),
                kind: Kind::Regular,
            },
        ];
        for path in paths {
            b.push(path);
        }
        b.build().unwrap();
    }

    #[test]
    fn test_redirects() {
        let mut b: TreeBuilder<CustomFile> = TreeBuilder::new();
        let paths = vec![
            CustomFile {
                path: "/usr/lib".into(),
                kind: Kind::Directory,
            },
            CustomFile {
                path: "/usr/lib64".into(),
                kind: Kind::Symlink("/usr/lib".into()),
            },
            CustomFile {
                path: "/usr/lib64/libz.so.1.2.13".into(),
                kind: Kind::Regular,
            },
            CustomFile {
                path: "/usr/lib64/libz.so.1".into(),
                kind: Kind::Symlink("libz.so.1.2.13".into()),
            },
            CustomFile {
                path: "/run/lock".into(),
                kind: Kind::Directory,
            },
            CustomFile {
                path: "/var/run/lock".into(),
                kind: Kind::Symlink("/run/lock".into()),
            },
            CustomFile {
                path: "/var/run/lock/subsys/1".into(),
                kind: Kind::Regular,
            },
        ];
        for path in paths {
            b.push(path);
        }
        let tree = b.build().unwrap();

        let dirs = tree
            .iter()
            .filter_map(|file| matches!(file.kind, Kind::Directory).then_some(file.path))
            .collect::<Vec<_>>();
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("/"),
                PathBuf::from("/run"),
                PathBuf::from("/run/lock"),
                PathBuf::from("/run/lock/subsys"),
                PathBuf::from("/usr"),
                PathBuf::from("/usr/lib"),
            ]
        );

        let symlink = tree
            .iter()
            .find(|file| matches!(file.kind, Kind::Symlink(_)))
            .unwrap();
        assert_eq!(
            symlink,
            CustomFile {
                path: "/usr/lib/libz.so.1".into(),
                kind: Kind::Symlink("libz.so.1.2.13".into()),
            }
        );
    }

    #[test]
    fn test_enumerate_leading_dirs() {
        let tests = [
            (
                PathBuf::from("/a/b/c/d/testing"),
                vec![
                    PathBuf::from("/"),
                    PathBuf::from("/a"),
                    PathBuf::from("/a/b"),
                    PathBuf::from("/a/b/c"),
                    PathBuf::from("/a/b/c/d"),
                ],
            ),
            (
                PathBuf::from("relative/path/testing"),
                vec![PathBuf::from("relative"), PathBuf::from("relative/path")],
            ),
            (PathBuf::from("no_parent"), vec![]),
        ];

        for (path, expected) in tests {
            let leading_dirs = enumerate_leading_dirs(&path);

            assert_eq!(leading_dirs, expected);
        }
    }
}
