use super::DAG;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, RwLock,
};

#[derive(Clone, Debug)]
struct ComponentNode {
    stack_idx: usize,
    stacked: bool,
}

pub struct TarjanStronglyConnectedComponents<'a, I>
where
    // I: Clone + Eq + Hash + PartialEq + Send + Sync + 'static,
    // I: Clone + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    graph: &'a DAG<I>,
    stack: Vec<&'a I>,
    nodes: Vec<ComponentNode>,
    seen: HashMap<&'a I, usize>,
    components: Vec<Vec<&'a I>>,
}

impl<'a, I> TarjanStronglyConnectedComponents<'a, I>
where
    I: Clone + Eq + Hash + PartialEq + Send + Sync + 'static,
{
    pub fn new(graph: &'a DAG<I>) -> TarjanStronglyConnectedComponents<I> {
        TarjanStronglyConnectedComponents {
            graph,
            stack: Vec::<&I>::new(),
            nodes: Vec::<ComponentNode>::with_capacity(graph.len()),
            seen: HashMap::<&I, usize>::new(),
            components: Vec::<Vec<&I>>::new(),
        }
    }

    pub fn has_circles(&'a mut self) -> bool {
        // start depth first search from each node that has not yet been visited
        for node in self.graph.keys() {
            if !self.seen.contains_key(&node) {
                self.dfs(&node);
            }
        }
        self.components.len() != self.graph.len()
    }

    fn dfs(&mut self, node: &'a I) -> &ComponentNode {
        let stack_idx = self.nodes.len();
        self.seen.insert(node, stack_idx);
        self.stack.push(node);
        self.nodes.push(ComponentNode {
            stack_idx,     // the index of the node on the stack
            stacked: true, // the node is currently on the stack
        });

        if let Some(links) = self.graph.get(node) {
            for neighbour in links {
                match self.seen.get(neighbour) {
                    Some(&i) => {
                        // node was already visited
                        if self.nodes[i].stacked {
                            self.nodes[stack_idx].stack_idx =
                                self.nodes[stack_idx].stack_idx.min(i);
                        }
                    }
                    None => {
                        // node has not yet been visited
                        let n = self.dfs(neighbour);
                        let n_stack_idx = n.stack_idx;
                        self.nodes[stack_idx].stack_idx =
                            self.nodes[stack_idx].stack_idx.min(n_stack_idx);
                    }
                };
            }
        }
        // maintain the stack invariant:
        // a node remains on the stack after it has been visited
        // iff there exists a path in the input graph from it some
        // node earlier on the stack
        if self.nodes[stack_idx].stack_idx == stack_idx {
            let mut circle = Vec::<&I>::new();
            let mut i = self.stack.len() - 1;
            loop {
                let w = self.stack[i];
                let n_stack_idx = self.seen[w];
                self.nodes[n_stack_idx].stacked = false;
                circle.push(w);
                if n_stack_idx == stack_idx {
                    break;
                };
                i -= 1;
            }
            self.stack.pop();
            self.components.push(circle);
        }
        &self.nodes[stack_idx]
    }
}
