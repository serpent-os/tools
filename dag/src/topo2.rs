use std::collections::{HashMap, VecDeque};

use ::petgraph::visit::VisitMap;
use petgraph::{
    visit::{Dfs, GraphRef, IntoNeighbors, IntoNeighborsDirected, Visitable, Walker},
    Graph,
};

/// A topological order traversal for a graph.
///
/// **Note** that `Topo` only visits nodes that are not part of cycles,
/// i.e. nodes in a true DAG. Use other visitors like `DfsPostOrder` or
/// algorithms like kosaraju_scc to handle graphs with possible cycles.
#[derive(Clone)]
pub struct Topo<N, VM> {
    queue: VecDeque<N>,
    indeg: HashMap<N, usize>,
    ordered: VM,
}

impl<N, VM> Topo<N, VM>
where
    N: Copy + std::hash::Hash + PartialEq + Eq,
    VM: VisitMap<N>,
{
    pub fn new<G>(graph: G, start_nodes: &Vec<N>) -> Self
    where
        G: GraphRef + Visitable<NodeId = N, Map = VM> + IntoNeighborsDirected,
    {
        let mut indeg = HashMap::new();
        let mut dfs = Dfs::empty(graph);

        for start in start_nodes {
            dfs = Dfs::from_parts(vec![*start], dfs.discovered);
            while let Some(node) = dfs.next(graph) {
                for adj in graph.neighbors(node) {
                    if indeg.contains_key(&adj) {
                        if let Some(cnt) = indeg.get_mut(&adj) {
                            (*cnt) += 1;
                        } else {
                            indeg.insert(adj, 1);
                        }
                    }
                }
            }
        }

        Topo {
            queue: VecDeque::from_iter(
                start_nodes
                    .iter()
                    .filter(|&node| indeg.get(node).is_some_and(|cnt| *cnt == 0))
                    .cloned(),
            ),
            indeg,
            ordered: graph.visit_map(),
        }
    }

    pub fn next<G>(&mut self, graph: G) -> Option<N>
    where
        G: IntoNeighborsDirected + Visitable<NodeId = N, Map = VM>,
    {
        if let Some(node) = self.queue.pop_front() {
            for adj in graph.neighbors(node) {
                if let Some(cnt) = self.indeg.get_mut(&adj) {
                    (*cnt) -= 1;
                    assert!((*cnt) >= 0);
                    if *cnt == 0 {
                        self.queue.push_back(adj);
                    }
                }
            }
            Some(node)
        } else {
            None
        }
    }
}

impl<G> Walker<G> for Topo<G::NodeId, G::Map>
where
    G: IntoNeighborsDirected + Visitable,
{
    type Item = G::NodeId;

    fn walk_next(&mut self, context: G) -> Option<Self::Item> {
        self.next(context)
    }
}
