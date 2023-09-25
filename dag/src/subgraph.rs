// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use petgraph::{prelude::GraphMap, visit::Dfs, EdgeType};

/// Given an input [GraphMap] and the start nodes, construct a subgraph
/// Used largely in transposed form for reverse dependency calculation
pub fn subgraph<V, E, Ty>(graph: &GraphMap<V, E, Ty>, starting_nodes: Vec<V>) -> GraphMap<V, E, Ty>
where
    V: Eq + std::hash::Hash + Ord + Copy,
    E: Default,
    Ty: EdgeType,
{
    let mut res = GraphMap::default();

    let mut dfs = Dfs::empty(&graph);
    for starting_node in starting_nodes {
        dfs.move_to(starting_node);
        while let Some(node) = dfs.next(&graph) {
            res.extend(
                graph
                    .neighbors_directed(node, petgraph::Direction::Outgoing)
                    .map(|adj| (node, adj)),
            );
        }
    }

    res
}

#[cfg(test)]
mod test {
    use petgraph::{
        prelude::DiGraphMap,
        visit::{Reversed, Topo, Walker},
    };

    use super::*;

    #[test]
    fn basic_topo() {
        let graph: DiGraphMap<i32, ()> = DiGraphMap::from_edges(&[(1, 2), (1, 3), (2, 3)]);
        let subg = subgraph(&graph, vec![1]);
        let topo = Topo::new(&subg);
        let order: Vec<i32> = topo.iter(&subg).collect();

        assert_eq!(order, vec![1, 2, 3]);
    }

    #[test]
    fn reverse_topo() {
        let graph: DiGraphMap<i32, ()> = DiGraphMap::from_edges(&[(1, 2), (1, 3), (2, 3)]);
        let items = vec![1];
        let subg = subgraph(&graph, items);
        let revg = Reversed(&subg);
        let removal: Vec<i32> = Topo::new(revg).iter(revg).collect();
        assert_eq!(removal, vec![3, 2, 1]);
    }

    // TODO: break cycles!
    #[ignore = "cycles breaking needs to be implemented"]
    #[test]
    fn cyclic_topo() {
        let graph: DiGraphMap<i32, ()> =
            DiGraphMap::from_edges(&[(1, 2), (1, 3), (2, 4), (2, 5), (3, 5), (4, 1)]);
        let items = vec![1, 4];
        let subg = subgraph(&graph, items);
        let revg = Reversed(&subg);
        let removal: Vec<i32> = Topo::new(revg).iter(revg).collect();
        assert_eq!(removal, vec![5, 3, 4, 2, 1]);
    }
}
