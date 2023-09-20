use petgraph::{prelude::GraphMap, visit::Dfs, EdgeType};

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
            for adj in graph.neighbors(node) {
                res.add_edge(node, adj, E::default());
            }
        }
    }

    res
}

#[cfg(test)]
mod test {
    use petgraph::{
        prelude::DiGraphMap,
        visit::{Topo, Walker},
    };

    use super::*;

    #[test]
    fn basic_topo() {
        let graph: DiGraphMap<i32, ()> = DiGraphMap::from_edges(&[(1, 2), (1, 3), (2, 3)]);
        let subgraph = subgraph(&graph, vec![1]);
        let topo = Topo::new(&subgraph);
        let order: Vec<i32> = topo.iter(&subgraph).collect();

        assert_eq!(order, vec![1, 2, 3]);
    }
}
