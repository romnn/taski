use crate::dfs::Dfs;

use std::collections::{HashMap, HashSet};

pub type DAG<N> = HashMap<N, HashSet<N>>;

pub trait Traversal<N> {
    fn traverse<'a, D, F>(&'a self, root: &'a N, depth: D, filter: F) -> Dfs<'a, N, F>
    where
        F: Fn(&&N) -> bool,
        N: std::hash::Hash + Eq,
        D: Into<Option<usize>>;
}

impl<N> Traversal<N> for DAG<N> {
    fn traverse<'a, D, F>(&'a self, root: &'a N, depth: D, filter: F) -> Dfs<'a, N, F>
    where
        F: Fn(&&N) -> bool,
        N: std::hash::Hash + Eq,
        D: Into<Option<usize>>,
    {
        Dfs::new(self, root, depth.into(), filter)
    }
}
