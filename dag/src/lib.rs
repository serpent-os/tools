// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use petgraph::{
    prelude::DiGraph,
    visit::{Dfs, Topo, Walker},
};

use self::subgraph::subgraph;

mod subgraph;

pub type NodeIndex = petgraph::prelude::NodeIndex<u32>;

#[derive(Debug, Clone)]
pub struct Dag<N>(DiGraph<N, (), u32>);

impl<N> Default for Dag<N> {
    fn default() -> Self {
        Self(DiGraph::default())
    }
}

impl<N> Dag<N>
where
    N: Clone + PartialEq,
{
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds node N to the graph and retusn it's index.
    /// If N already exists, it'll return the index of that node.
    pub fn add_node_or_get_index(&mut self, node: N) -> NodeIndex {
        if let Some(index) = self.get_index(&node) {
            index
        } else {
            self.0.add_node(node)
        }
    }

    pub fn node_exists(&self, node: &N) -> bool {
        self.get_index(node).is_some()
    }

    pub fn remove_node(&mut self, node: &N) -> Option<N> {
        if let Some(index) = self.get_index(node) {
            self.0.remove_node(index)
        } else {
            None
        }
    }

    pub fn add_edge(&mut self, a: NodeIndex, b: NodeIndex) -> bool {
        let a_node = &self.0[a];

        // prevent cycle (b connects to a)
        if self.dfs(b).any(|n| n == a_node) {
            return false;
        }

        // don't add edge if it alread exists
        if self.0.find_edge(a, b).is_some() {
            return false;
        }

        // We're good, add it
        self.0.add_edge(a, b, ());

        true
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = &'_ N> {
        self.0.node_indices().map(|i| &self.0[i])
    }

    pub fn dfs(&self, start: NodeIndex) -> impl Iterator<Item = &'_ N> {
        let dfs = Dfs::new(&self.0, start);

        dfs.iter(&self.0).map(|i| &self.0[i])
    }

    pub fn topo(&self) -> impl Iterator<Item = &'_ N> {
        let topo = Topo::new(&self.0);

        topo.iter(&self.0).map(|i| &self.0[i])
    }

    pub fn transpose(&self) -> Self {
        let mut transposed = self.0.clone();
        transposed.reverse();
        Self(transposed)
    }

    pub fn subgraph(&self, starting_nodes: &[N]) -> Self {
        Self(subgraph(&self.0, starting_nodes))
    }

    pub fn get_index(&self, node: &N) -> Option<NodeIndex> {
        self.0.node_indices().find(|i| self.0[*i] == *node)
    }
}
