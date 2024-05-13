use petgraph as pg;

// use crate::dfs::Dfs;

// use std::collections::{HashMap, HashSet};
//
// pub type DAG<N> = HashMap<N, HashSet<N>>;
pub type Idx = pg::graph::NodeIndex<usize>;
pub type DAG<N> = pg::stable_graph::StableDiGraph<N, (), usize>;
// pub type DAG<N> = pg::graphmap::DiGraphMap<N, ()>;
// pub type DAG<N> = pg::graph::DiGraph<N, (), usize>;
//
// pub trait Traversal<N> {
//     fn traverse<'a, D, F>(&'a self, root: &'a N, depth: D, filter: F) -> Dfs<'a, N, F>
//     where
//         F: Fn(&&N) -> bool,
//         N: std::hash::Hash + Eq,
//         D: Into<Option<usize>>;
// }
//
// impl<N> Traversal<N> for DAG<N> {
//     fn traverse<'a, D, F>(&'a self, root: &'a N, depth: D, filter: F) -> Dfs<'a, N, F>
//     where
//         F: Fn(&&N) -> bool,
//         N: std::hash::Hash + Eq,
//         D: Into<Option<usize>>,
//     {
//         Dfs::new(self, root, depth.into(), filter)
//     }
// }

// #[derive(Clone)]
// pub struct Dfs<'a, N, F> {
//     stack: Vec<(usize, &'a N)>,
//     graph: &'a DAG<N>,
//     max_depth: Option<usize>,
//     filter: F,
// }

/// **Note:** The algorithm may not behave correctly if nodes are removed
/// during iteration. It may not necessarily visit added nodes or edges.
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
    // Create a new **Dfs** using the graph's visitor map, and no stack.
    // pub fn empty<G>(graph: G) -> Self
    // where
    //     G: pg::visit::GraphRef + pg::visit::Visitable<NodeId = N, Map = VM>,
    // {
    //     Dfs {
    //         stack: Vec::new(),
    //         discovered: graph.visit_map(),
    //     }
    // }

    /// Create a new **Dfs**, using the graph's visitor map,
    /// and put **start** in the stack of nodes to visit.
    pub fn new<G>(graph: G, start: N) -> Self
    where
        G: pg::visit::GraphRef + pg::visit::Visitable<NodeId = N, Map = VM>,
    {
        // let mut dfs = Dfs::empty(graph);
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

    pub fn filter(mut self, filter: F) -> Self
    where
        F: Fn(&N) -> bool,
    {
        self.filter = Some(filter);
        self
    }

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
            let max_depth_reached = self
                .max_depth
                .map(|max_depth| depth >= max_depth)
                .unwrap_or(false);
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

// impl<'a, N, F> Dfs<'a, N, F>
// where
//     N: std::hash::Hash + Eq,
//     F: Fn(&&N) -> bool,
// {
//     #[inline]
//     pub fn new(
//         graph: &'a DAG<N>,
//         root: &'a N,
//         max_depth: impl Into<Option<usize>>,
//         filter: F,
//     ) -> Self {
//         let mut stack = vec![];
//         if let Some(children) = graph.get(root) {
//             stack.extend(children.iter().map(|child| (1, child)));
//         }
//         Self {
//             stack,
//             graph,
//             max_depth: max_depth.into(),
//             filter,
//         }
//     }
//
//     // this gives lifetime errors
//     // fn children(&self, node: &'a N) -> Option<impl Iterator<Item = &'a N> + '_> {
//     //     self.graph
//     //         .get(&node)
//     //         .map(|children| children.iter().filter(&self.filter))
//     // }
// }
//
// impl<'a, N, F> Iterator for Dfs<'a, N, F>
// where
//     N: std::hash::Hash + Eq,
//     F: Fn(&&N) -> bool,
// {
//     type Item = (usize, &'a N);
//
//     #[inline]
//     fn next(&mut self) -> Option<Self::Item> {
//         match self.stack.pop() {
//             Some((depth, node)) => {
//                 if let Some(max_depth) = self.max_depth {
//                     if depth >= max_depth {
//                         return Some((depth, node));
//                     }
//                 }
//                 if let Some(children) = self.graph.get(node) {
//                     self.stack.extend(
//                         children
//                             .iter()
//                             .filter(&self.filter)
//                             .map(|child| (depth + 1, child)),
//                     );
//                 };
//                 Some((depth, node))
//             }
//             None => None,
//         }
//     }
// }

// #[derive(Clone)]
// pub struct Dfs<'a, N, F> {
//     stack: Vec<(usize, &'a N)>,
//     graph: &'a DAG<N>,
//     max_depth: Option<usize>,
//     filter: F,
// }
//
// impl<'a, N, F> Dfs<'a, N, F>
// where
//     N: std::hash::Hash + Eq,
//     F: Fn(&&N) -> bool,
// {
//     #[inline]
//     pub fn new(
//         graph: &'a DAG<N>,
//         root: &'a N,
//         max_depth: impl Into<Option<usize>>,
//         filter: F,
//     ) -> Self {
//         let mut stack = vec![];
//         if let Some(children) = graph.get(root) {
//             stack.extend(children.iter().map(|child| (1, child)));
//         }
//         Self {
//             stack,
//             graph,
//             max_depth: max_depth.into(),
//             filter,
//         }
//     }
//
//     // this gives lifetime errors
//     // fn children(&self, node: &'a N) -> Option<impl Iterator<Item = &'a N> + '_> {
//     //     self.graph
//     //         .get(&node)
//     //         .map(|children| children.iter().filter(&self.filter))
//     // }
// }
//
// impl<'a, N, F> Iterator for Dfs<'a, N, F>
// where
//     N: std::hash::Hash + Eq,
//     F: Fn(&&N) -> bool,
// {
//     type Item = (usize, &'a N);
//
//     #[inline]
//     fn next(&mut self) -> Option<Self::Item> {
//         match self.stack.pop() {
//             Some((depth, node)) => {
//                 if let Some(max_depth) = self.max_depth {
//                     if depth >= max_depth {
//                         return Some((depth, node));
//                     }
//                 }
//                 if let Some(children) = self.graph.get(node) {
//                     self.stack.extend(
//                         children
//                             .iter()
//                             .filter(&self.filter)
//                             .map(|child| (depth + 1, child)),
//                     );
//                 };
//                 Some((depth, node))
//             }
//             None => None,
//         }
//     }
// }
