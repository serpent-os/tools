use ::petgraph::visit::VisitMap;
use petgraph::{
    visit::{
        Dfs, GraphRef, IntoNeighbors, IntoNeighborsDirected, IntoNodeIdentifiers, Reversed,
        Visitable,
    },
    Direction::Incoming,
};

/// A topological order traversal for a graph.
///
/// **Note** that `Topo` only visits nodes that are not part of cycles,
/// i.e. nodes in a true DAG. Use other visitors like `DfsPostOrder` or
/// algorithms like kosaraju_scc to handle graphs with possible cycles.
#[derive(Clone)]
pub struct Topo<N, VM> {
    tovisit: Vec<N>,
    ordered: VM,
}

impl<N, VM> Default for Topo<N, VM>
where
    VM: Default,
{
    fn default() -> Self {
        Topo {
            tovisit: Vec::new(),
            ordered: VM::default(),
        }
    }
}

impl<N, VM> Topo<N, VM>
where
    N: Copy + PartialEq,
    VM: VisitMap<N>,
{
    /// Create a new `Topo`, using the graph's visitor map, and put all
    /// initial nodes in the to visit list.
    pub fn new<G>(graph: G, starts: Option<Vec<N>>) -> Self
    where
        G: IntoNodeIdentifiers + IntoNeighborsDirected + Visitable<NodeId = N, Map = VM>,
    {
        let mut topo = Self::empty(graph);
        if let Some(starting_nodes) = starts {
            topo.extend_with_requested_initials(graph, &starting_nodes)
        } else {
            topo.extend_with_initials(graph);
        }
        topo
    }

    fn extend_with_initials<G>(&mut self, g: G)
    where
        G: IntoNodeIdentifiers + IntoNeighborsDirected<NodeId = N>,
    {
        // find all initial nodes (nodes without incoming edges)
        self.tovisit.extend(
            g.node_identifiers()
                .filter(move |&a| g.neighbors_directed(a, Incoming).next().is_none()),
        );
    }

    fn extend_with_requested_initials<G>(&mut self, g: G, starting_nodes: &Vec<N>)
    where
        G: IntoNodeIdentifiers
            + IntoNeighborsDirected<NodeId = N>
            + Visitable<NodeId = N, Map = VM>,
    {
        let mut dfs = Dfs::empty(g);
        for &node in starting_nodes {
            dfs.move_to(node);
            while let Some(_) = dfs.next(g) {}
        }

        for node in g.node_identifiers() {
            if !self.ordered.is_visited(&node) {
                assert!(!self.ordered.visit(node));
            }
        }
        self.tovisit.extend(starting_nodes.iter().filter(|&&node| {
            g.neighbors_directed(node, Incoming)
                .all(|adj| self.ordered.is_visited(&adj))
        }));
    }

    /* Private until it has a use */
    /// Create a new `Topo`, using the graph's visitor map with *no* starting
    /// index specified.
    fn empty<G>(graph: G) -> Self
    where
        G: GraphRef + Visitable<NodeId = N, Map = VM>,
    {
        Topo {
            ordered: graph.visit_map(),
            tovisit: Vec::new(),
        }
    }

    /// Clear visited state, and put all initial nodes in the to visit list.
    pub fn reset<G>(&mut self, graph: G)
    where
        G: IntoNodeIdentifiers + IntoNeighborsDirected + Visitable<NodeId = N, Map = VM>,
    {
        graph.reset_map(&mut self.ordered);
        self.tovisit.clear();
        self.extend_with_initials(graph);
    }

    /// Return the next node in the current topological order traversal, or
    /// `None` if the traversal is at the end.
    ///
    /// *Note:* The graph may not have a complete topological order, and the only
    /// way to know is to run the whole traversal and make sure it visits every node.
    pub fn next<G>(&mut self, g: G) -> Option<N>
    where
        G: IntoNeighborsDirected + Visitable<NodeId = N, Map = VM>,
    {
        // Take an unvisited element and find which of its neighbors are next
        while let Some(nix) = self.tovisit.pop() {
            if self.ordered.is_visited(&nix) {
                continue;
            }
            self.ordered.visit(nix);
            for neigh in g.neighbors(nix) {
                // Look at each neighbor, and those that only have incoming edges
                // from the already ordered list, they are the next to visit.
                if Reversed(g)
                    .neighbors(neigh)
                    .all(|b| self.ordered.is_visited(&b))
                {
                    self.tovisit.push(neigh);
                }
            }
            return Some(nix);
        }
        None
    }
}
