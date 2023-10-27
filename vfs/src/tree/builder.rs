// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Build a vfs tree incrementally
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use crate::tree::{Kind, Tree};

use super::{BlitFile, Error};

/// Builder used to generate a full tree, free of conflicts
pub struct TreeBuilder<T: BlitFile> {
    // Explicitly requested incoming paths
    explicit: Vec<T>,

    // Implicitly created paths
    implicit_dirs: BTreeMap<PathBuf, T>,
}

/// Special sort algorithm for files by directory
fn sorted_paths<T: BlitFile>(a: &T, b: &T) -> std::cmp::Ordering {
    let a_path_len = a.path().to_string_lossy().to_string().matches('/').count();
    let b_path_len = b.path().to_string_lossy().to_string().matches('/').count();
    if a_path_len != b_path_len {
        a_path_len.cmp(&b_path_len)
    } else {
        a.path().cmp(&b.path())
    }
}

impl<T: BlitFile> Default for TreeBuilder<T> {
    fn default() -> Self {
        TreeBuilder::new()
    }
}

impl<T: BlitFile> TreeBuilder<T> {
    pub fn new() -> Self {
        TreeBuilder {
            explicit: vec![],
            implicit_dirs: BTreeMap::new(),
        }
    }

    /// Push an item to the builder - we don't care if we have duplicates yet
    pub fn push(&mut self, item: T) {
        let path = item.path();

        // Find all parent paths
        if let Some(parent) = path.parent() {
            let mut leading_path: Option<PathBuf> = None;
            // Build a set of parent paths skipping `/`, yielding `usr`, `usr/bin`, etc.
            let components = parent
                .components()
                .map(|p| p.as_os_str().to_string_lossy().to_string())
                //.skip(1)
                .collect::<Vec<_>>();
            for component in components {
                let full_path = match leading_path {
                    Some(fp) => fp.join(&component),
                    None => PathBuf::from(&component),
                };
                leading_path = Some(full_path.clone());
                self.implicit_dirs
                    .insert(full_path.clone(), full_path.into());
            }
        }
        self.explicit.push(item);
    }

    /// Sort incoming entries and remove duplicates
    pub fn bake(&mut self) {
        self.explicit.sort_by(sorted_paths);

        // Walk again to remove accidental dupes
        for i in self.explicit.iter() {
            self.implicit_dirs.remove(&i.path());
        }
    }

    /// Generate the final tree by baking all inputs
    pub fn tree(&self) -> Result<Tree<T>, Error> {
        // Chain all directories, replace implicits with explicits
        let all_dirs = self
            .explicit
            .iter()
            .filter(|f| matches!(f.kind(), Kind::Directory))
            .chain(self.implicit_dirs.values())
            .map(|d| (d.path().to_string_lossy().to_string(), d))
            .collect::<BTreeMap<_, _>>();

        // build a set of redirects
        let mut redirects = HashMap::new();

        // Resolve symlinks-to-dirs
        for link in self.explicit.iter() {
            if let Kind::Symlink(target) = link.kind() {
                let path = link.path();

                // Resolve the link.
                let target = if target.starts_with('/') {
                    target.into()
                } else {
                    let parent = path.parent();
                    if let Some(parent) = parent {
                        parent.join(target)
                    } else {
                        target.into()
                    }
                };
                let string_path = path.to_string_lossy().to_string();
                let string_target = target.to_string_lossy().to_string();
                if all_dirs.get(&string_target).is_some() {
                    redirects.insert(string_path, string_target);
                }
            }
        }

        // Insert everything WITHOUT redirects, directory first.
        let mut full_set = all_dirs
            .into_values()
            .chain(
                self.explicit
                    .iter()
                    .filter(|m| !matches!(m.kind(), Kind::Directory)),
            )
            .collect::<Vec<_>>();
        full_set.sort_by(|a, b| sorted_paths(*a, *b));

        let mut tree: Tree<T> = Tree::new();

        // Build the initial full tree now.
        for entry in full_set {
            // New node for this guy
            let path = entry.path();
            let node = tree.new_node(entry.clone());

            if let Some(parent) = path.parent() {
                tree.add_child_to_node(node, parent)?;
            }
        }

        // Reparent any symlink redirects.
        for (source_tree, target_tree) in redirects {
            tree.reparent(source_tree, target_tree)?;
        }
        Ok(tree)
    }
}

#[cfg(test)]
mod tests {
    use super::{BlitFile, TreeBuilder};
    use crate::tree::Kind;
    use std::path::PathBuf;

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
        b.bake();
        b.tree().unwrap();
    }
}
