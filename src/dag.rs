use petgraph as pg;

pub type Idx = pg::graph::NodeIndex<usize>;
pub type DAG<N> = pg::stable_graph::StableDiGraph<N, (), usize>;

/// Depth first search.
///
/// **Note:**
/// The algorithm may not behave correctly if nodes are
/// removed during iteration.
/// It may not necessarily visit added nodes or edges.
#[derive(Clone, Debug)]
pub struct Dfs<N, VM, F> {
    /// The stack of nodes to visit
    pub stack: Vec<(usize, N)>,
    /// The map of discovered nodes
    pub discovered: VM,
    /// Maximum depth of the traversal
    pub max_depth: Option<usize>,
    /// Filter nodes to traverse
    pub filter: Option<F>,
}

impl<N, VM, F> Dfs<N, VM, F>
where
    N: Copy + PartialEq,
    VM: pg::visit::VisitMap<N>,
{
    /// Create a new **Dfs**, using the graph's visitor map,
    /// and put **start** in the stack of nodes to visit.
    pub fn new<G>(graph: G, start: N) -> Self
    where
        G: pg::visit::GraphRef + pg::visit::Visitable<NodeId = N, Map = VM>,
    {
        let mut dfs = Self {
            stack: Vec::new(),
            discovered: graph.visit_map(),
            filter: None,
            max_depth: None,
        };
        dfs.stack.clear();
        dfs.stack.push((0, start));
        dfs
    }

    #[must_use]
    pub fn filter(mut self, filter: F) -> Self
    where
        F: Fn(&N) -> bool,
    {
        self.filter = Some(filter);
        self
    }

    #[must_use]
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Return the next node in the dfs,
    /// or **None** if the traversal is done.
    pub fn next<G>(&mut self, graph: G) -> Option<N>
    where
        G: pg::visit::IntoNeighbors<NodeId = N>,
        F: Fn(&N) -> bool,
    {
        while let Some((depth, node)) = self.stack.pop() {
            let max_depth_reached = self.max_depth.is_some_and(|max_depth| depth >= max_depth);
            let first_visit = self.discovered.visit(node);

            if first_visit && !max_depth_reached {
                let neighbors = graph
                    .neighbors(node)
                    .filter(|n| match self.filter.as_ref() {
                        Some(filter) => filter(n),
                        None => true,
                    });
                for succ in neighbors {
                    if !self.discovered.is_visited(&succ) {
                        self.stack.push((depth + 1, succ));
                    }
                }
                return Some(node);
            }
        }
        None
    }
}
