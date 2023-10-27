// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Virtual filesystem tree (optimise layout inserts)

use core::fmt::Debug;
use std::{collections::HashMap, path::PathBuf, vec};

use indextree::{Arena, NodeId};
use thiserror::Error;
pub mod builder;

#[derive(Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    // Regular path
    Regular,

    // Directory (parenting node)
    #[default]
    Directory,

    // Symlink to somewhere else.
    Symlink(String),
}

#[derive(Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Node<T: BlitFile> {
    /// The partial file name (usr, etc)
    name: String,
    file: T,
}

/// Simple generic interface for blittable files while retaining details
/// All implementations should return a directory typed blitfile for a PathBuf
pub trait BlitFile: Clone + Sized + Debug + From<PathBuf> {
    fn kind(&self) -> Kind;
    fn path(&self) -> PathBuf;

    /// Clone the BlitFile and update the path
    fn cloned_to(&self, path: PathBuf) -> Self;
}

/// Actual tree implementation, encapsulating indextree
#[derive(Debug)]
pub struct Tree<T: BlitFile> {
    arena: Arena<T>,
    map: HashMap<PathBuf, NodeId>,
}

impl<T: BlitFile> Tree<T> {
    /// Construct a new Tree
    fn new() -> Self {
        Tree {
            arena: Arena::new(),
            map: HashMap::new(),
        }
    }

    /// Generate a new node, store the path mapping for it
    fn new_node(&mut self, data: T) -> NodeId {
        let path = data.path();
        let node = self.arena.new_node(data);
        self.map.insert(path, node);
        node
    }

    /// Resolve a node using the path
    fn resolve_node(&self, data: impl Into<PathBuf>) -> Option<&NodeId> {
        self.map.get(&data.into())
    }

    /// Add a child to the given parent node
    fn add_child_to_node(
        &mut self,
        node_id: NodeId,
        parent: impl Into<PathBuf>,
    ) -> Result<(), Error> {
        let parent = parent.into();
        let node = self.arena.get(node_id).unwrap();
        if let Some(parent_node) = self.map.get(&parent) {
            let others = parent_node
                .children(&self.arena)
                .filter_map(|n| self.arena.get(n))
                .filter_map(|n| {
                    if n.get().path().file_name() == node.get().path().file_name() {
                        Some(n.get())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if !others.is_empty() {
                Err(Error::Duplicate(node.get().path()))
            } else {
                parent_node.append(node_id, &mut self.arena);
                Ok(())
            }
        } else {
            Err(Error::MissingParent(parent.clone()))
        }
    }

    pub fn print(&self) {
        let root = self.resolve_node("/").unwrap();
        eprintln!("{:#?}", root.debug_pretty_print(&self.arena));
    }

    /// For all descendents of the given source tree, return a set of the reparented nodes,
    /// and remove the originals from the tree
    fn reparent(
        &mut self,
        source_tree: impl Into<PathBuf>,
        target_tree: impl Into<PathBuf>,
    ) -> Result<(), Error> {
        let source_path = source_tree.into();
        let target_path = target_tree.into();
        let mut mutations = vec![];
        let mut orphans = vec![];
        if let Some(source) = self.map.get(&source_path) {
            if let Some(_target) = self.map.get(&target_path) {
                for child in source.descendants(&self.arena).skip(1) {
                    mutations.push(child);
                }
            }

            for i in mutations {
                let original = self.arena.get(i).unwrap().get();
                let relapath =
                    target_path.join(original.path().strip_prefix(&source_path).unwrap());
                orphans.push(original.cloned_to(relapath));
            }

            // Remove descendents
            let children = source.children(&self.arena).collect::<Vec<_>>();
            for child in children.iter() {
                child.remove_subtree(&mut self.arena)
            }
        }

        for orphan in orphans {
            let path = orphan.path().clone();
            // Do we have this node already?
            let node = match self.resolve_node(&path) {
                Some(n) => *n,
                None => self.new_node(orphan),
            };
            if let Some(parent) = path.parent() {
                self.add_child_to_node(node, parent)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing parent: {0}")]
    MissingParent(PathBuf),

    #[error("duplicate entry")]
    Duplicate(PathBuf),
}
