use super::{State, DAG};
use std::collections::HashMap;
use std::hash::Hash;

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct Dfs<'a, I>
where
    I: Clone,
{
    stack: Vec<(usize, &'a I)>,
    nodes: &'a DAG<I>,
    states: &'a HashMap<I, State>,
    max_depth: Option<usize>,
}

impl<'a, I> Dfs<'a, I>
where
    I: Hash + Eq,
    // I: Clone + Hash + Eq,
{
    #[inline]
    pub fn new(
        root: &'a I,
        nodes: &'a DAG<I>,
        states: &'a HashMap<I, State>,
        max_depth: impl Into<Option<usize>>,
    ) -> Self {
        let mut stack = Vec::default();
        // vec![];
        // if nodes.contains_key(root) {
        //     stack.push((0, root.clone()));
        // }
        if let Some(children) = nodes.get(root) {
            stack.extend(children.iter().map(|child| (1, child)));
        }
        Self {
            stack,
            nodes,
            states,
            max_depth: max_depth.into(),
        }
    }

    pub fn states(self) -> impl Iterator<Item = (usize, &'a I, Option<&'a State>)> + 'a {
        self.clone().map(move |(depth, dep)| {
            let state = self.states.get(&dep);
            (depth, dep, state)
        })
    }
}

impl<'a, I> Iterator for Dfs<'a, I>
where
    I: Hash + Eq,
    // I: Clone + Hash + Eq,
{
    type Item = (usize, &'a I);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.stack.pop() {
            Some((depth, node)) => {
                if let Some(max_depth) = self.max_depth {
                    if depth >= max_depth {
                        return Some((depth, node));
                    }
                }
                if let Some(children) = self.nodes.get(&node) {
                    self.stack
                        .extend(children.iter().map(|child| (depth + 1, child)));
                };
                Some((depth, node))
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;
    use pretty_assertions::assert_eq;
    use std::collections::{HashMap, HashSet};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_default_scheduler() {
        let graph: DAG<u32> = HashMap::from_iter([
            (1, [2, 3, 4].into_iter().collect::<HashSet<u32>>()),
            (2, [5].into_iter().collect::<HashSet<u32>>()),
            (3, [5].into_iter().collect::<HashSet<u32>>()),
        ]);
        let states = graph
            .keys()
            .map(|key| (key.clone(), State::Pending))
            .collect::<HashMap<_, _>>();
        let dfs = Dfs::new(&1, &graph, &states, None);
        assert_eq!(
            dfs.collect::<Vec<_>>()
                .into_iter()
                .sorted()
                .collect::<Vec<_>>(),
            [(1, &2), (2, &5), (1, &4), (1, &3), (2, &5)]
                .into_iter()
                .sorted()
                .collect::<Vec<_>>()
        );
    }
}
