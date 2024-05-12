#![allow(warnings)]

pub mod cycles;
pub mod dfs;

use dfs::Dfs;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(thiserror::Error, Debug, Clone)]
// pub enum ScheduleError<I>
pub enum Error
// where
//     I: std::fmt::Debug,
{
    #[error("dependency cycle detected")]
    Cycle,
    // #[error("new dependencies: `{0:?}`")]
    // NewDependencies(HashSet<I>),

    // #[error("policy scheduled task that is not ready: `{0:?}`")]
    // BadPolicy(I),
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum State {
    Pending,
    Running,
    Success,
    Failed,
}

pub type DAG<I> = HashMap<I, HashSet<I>>;

pub struct Schedule<I> {
    ready: HashSet<I>,
    states: HashMap<I, State>,
    deps: DAG<I>,
    dependants: DAG<I>,
}

impl<I> Schedule<I> {
    pub fn new() -> Self {
        Self {
            ready: HashSet::default(),
            states: HashMap::default(),
            deps: HashMap::default(),
            dependants: HashMap::default(),
        }
    }
}

impl<I> Schedule<I>
where
    I: Clone + Eq + Hash + std::fmt::Debug + 'static,
{
    pub fn extend(&mut self, nodes: DAG<I>) -> Result<(), Error> {
        for (node, new_deps) in nodes.into_iter() {
            if !self.deps.contains_key(&node) {
                self.deps.insert(node.clone(), new_deps);
                self.states.insert(node.clone(), State::Pending);
            }

            if let Some(deps) = self.deps.get(&node) {
                if deps.is_empty() {
                    self.ready.insert(node.clone());
                }

                // this should be fine?
                for dep in deps {
                    let mut rev_deps = self
                        .dependants
                        .entry(dep.to_owned())
                        .or_insert(HashSet::new());
                    rev_deps.insert(node.clone());
                }
            }
        }
        // checking for cycles needs full dag, but we want to check first before making changes
        // todo: check for cycles
        Ok(())
    }

    pub fn dependencies<'a>(&'a self, node: &'a I) -> Dfs<'a, I> {
        Dfs::new(node, &self.deps, &self.states, Some(1))
    }

    pub fn dependants<'a>(&'a self, node: &'a I) -> Dfs<'a, I> {
        Dfs::new(node, &self.dependants, &self.states, Some(1))
    }

    pub fn recursive_dependencies<'a>(&'a self, node: &'a I) -> Dfs<'a, I> {
        Dfs::new(node, &self.deps, &self.states, None)
    }

    pub fn recursive_dependants<'a>(&'a self, node: &'a I) -> Dfs<'a, I> {
        Dfs::new(node, &self.dependants, &self.states, None)
    }
}
