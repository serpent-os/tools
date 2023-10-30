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
        if let Some(resolved_path) = resolve_symlink(&item) {
            self.symlinks.push(Symlink {
                item,
                resolved_path,
            });
            return;
        }

        // Find all leading directories and add them
        for dir in enumerate_leading_dirs(&item.path()) {
            self.entries.insert(Entry::Directory(dir.into()));
        }

        // Insert the provided entry
        self.entries.insert(Entry::new(item));
    }

    /// Process all symlinks, adding them to `entries`.
    ///
    /// If the symlink is a dir, resolve it and add as Entry::Directory
    /// If the symlink is not a dir, add it as Entry::Other
    fn process_symlinks(&mut self) {
        for Symlink {
            item,
            resolved_path,
        } in self.symlinks.drain(..)
        {
            let is_resolved_dir = self.entries.iter().any(|entry| {
                entry.inner().path() == resolved_path
                    && matches!(entry.inner().kind(), Kind::Directory)
            });

            // If this is a known directory, add it as the resolved directory
            if is_resolved_dir {
                let item = item.cloned_to(resolved_path);
                self.entries.insert(Entry::Directory(item));
            }
            // otherwise this is just a normal symlink
            else {
                self.entries.insert(Entry::Other(item));
            }
        }
    }

    /// Build a [`Tree`] from the provided items
    pub fn build(mut self) -> Result<Tree<T>, Error> {
        self.process_symlinks();

        self.entries
            .into_iter()
            .try_fold(Tree::new(), |mut tree, entry| {
                let entry = entry.into_inner();

                let path = entry.path();
                let node = tree.new_node(entry);

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
    resolved_path: PathBuf,
}

fn resolve_symlink<T: BlitFile>(item: &T) -> Option<PathBuf> {
    let Kind::Symlink(target) = item.kind() else {
        return None;
    };

    let path = item.path();

    // Resolve the link.
    let target = if target.starts_with('/') {
        // Absolute
        target.into()
    } else if let Some(parent) = path.parent() {
        // Relative w/ parent
        parent.join(target)
    } else {
        // Relative to root
        target.into()
    };

    Some(normalize_path(target))
}

// Remove `.` and `..` components
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
