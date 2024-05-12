use crate::dag::DAG;

#[derive(Clone)]
pub struct Dfs<'a, N, F> {
    stack: Vec<(usize, &'a N)>,
    graph: &'a DAG<N>,
    max_depth: Option<usize>,
    filter: F,
}

impl<'a, N, F> Dfs<'a, N, F>
where
    N: std::hash::Hash + Eq,
    F: Fn(&&N) -> bool,
{
    #[inline]
    pub fn new(
        graph: &'a DAG<N>,
        root: &'a N,
        max_depth: impl Into<Option<usize>>,
        filter: F,
    ) -> Self {
        let mut stack = vec![];
        if let Some(children) = graph.get(root) {
            stack.extend(children.iter().map(|child| (1, child)));
        }
        Self {
            stack,
            graph,
            max_depth: max_depth.into(),
            filter,
        }
    }

    // this gives lifetime errors
    // fn children(&self, node: &'a N) -> Option<impl Iterator<Item = &'a N> + '_> {
    //     self.graph
    //         .get(&node)
    //         .map(|children| children.iter().filter(&self.filter))
    // }
}

impl<'a, N, F> Iterator for Dfs<'a, N, F>
where
    N: std::hash::Hash + Eq,
    F: Fn(&&N) -> bool,
{
    type Item = (usize, &'a N);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.stack.pop() {
            Some((depth, node)) => {
                if let Some(max_depth) = self.max_depth {
                    if depth >= max_depth {
                        return Some((depth, node));
                    }
                }
                if let Some(children) = self.graph.get(node) {
                    self.stack.extend(
                        children
                            .iter()
                            .filter(&self.filter)
                            .map(|child| (depth + 1, child)),
                    );
                };
                Some((depth, node))
            }
            None => None,
        }
    }
}
