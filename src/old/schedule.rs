use super::error::{Error, ScheduleError};
use super::task::{IntoTask, Task, TaskNode};
use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use std::cell::RefCell;
use std::cmp::Eq;
use std::collections::hash_map::{Entry, HashMap};
use std::collections::HashSet;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Eq, PartialEq, Clone, Debug)]
// pub(super) enum State {
pub enum State {
    Pending,
    Running,
    Success,
    Failed,
}

// pub(super) type DAG<I> = HashMap<I, HashMap<I, State>>;
// pub type DAG<I> = HashMap<I, HashMap<I, State>>;
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
            ready: HashSet::new(),
            states: HashMap::new(),
            deps: HashMap::new(),
            dependants: HashMap::new(),
        }
    }
}

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct Dfs<'a, I> {
    stack: Vec<(usize, I)>,
    nodes: &'a DAG<I>,
    states: &'a HashMap<I, State>,
    max_depth: Option<usize>,
}

impl<'a, I> Dfs<'a, I>
where
    I: Clone + Hash + Eq,
{
    #[inline]
    pub fn new(
        root: &'a I,
        nodes: &'a DAG<I>,
        states: &'a HashMap<I, State>,
        max_depth: Option<usize>,
    ) -> Self {
        let mut stack = vec![];
        // if nodes.contains_key(root) {
        //     stack.push((0, root.clone()));
        // }
        if let Some(children) = nodes.get(root) {
            stack.extend(children.iter().map(|child| (1, child.clone())));
        }
        Self {
            stack,
            nodes,
            states,
            max_depth,
        }
    }

    pub fn states(self) -> impl Iterator<Item = (usize, I, Option<&'a State>)> + 'a {
        self.clone().map(move |(depth, dep)| {
            let state = self.states.get(&dep);
            (depth, dep, state)
        })
    }
}

impl<'a, I> Iterator for Dfs<'a, I>
where
    I: Clone + Hash + Eq,
{
    type Item = (usize, I);

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
                        .extend(children.iter().cloned().map(|child| (depth + 1, child)));
                };
                Some((depth, node))
            }
            None => None,
        }
    }
}

impl<I> Schedule<I>
where
    I: Clone + Eq + Hash + std::fmt::Debug + 'static,
{
    // pub fn add_task<T: IntoTask<I, C, O, E>>(&mut self, task: T) -> Result<(), ScheduleError<I>> {
    //     let deps: DAG<I> = HashMap::new();
    //     let mut seen = HashSet::<I>::new();
    //     let mut stack = Vec::<TaskNode<I, C, O, E>>::new();

    //     stack.push(Box::new(task).into_task());

    //     while let Some(current) = stack.pop() {
    //         seen.insert(current.task.id());
    //         let mut deps = deps.entry(current.task.id()).or_insert(HashSet::new());

    //         // consumes dependencies
    //         for dep in current.dependencies.into_iter() {
    //             let dep_task = dep.into_task();
    //             deps.insert(dep_task.task.id());
    //             if !seen.contains(&dep_task.task.id()) {
    //                 stack.push(dep_task);
    //             }
    //         }

    //         // consumes task
    //         self.tasks
    //             .insert(current.task.id(), State::Pending(current.task.task));
    //     }
    //     self.extend(deps)?;
    //     Ok(())
    // }

    pub fn extend(&mut self, nodes: DAG<I>) -> Result<(), ScheduleError<I>> {
        for (node, new_deps) in nodes.into_iter() {
            if !self.deps.contains_key(&node) {
                self.deps.insert(node.clone(), new_deps);
                self.states.insert(node.clone(), State::Pending);
            }

            // match self.deps.entry(node.clone()) {
            // match self.deps.get(&node) {
            //     // Entry::Occupied(_) => {
            //     Some(deps) => {
            //         // Err(ScheduleError::NewDependencies(diff))
            //         // if node existed, check if there are any new dependencies

            //         // let deps = self.deps.get(&node).unwrap();
            //         // let diff: HashSet<_> = new_deps.difference(&deps).cloned().collect();
            //         // if !diff.is_empty() {
            //         //     Err(ScheduleError::NewDependencies(diff))
            //         // } else {
            //         //     Ok(())
            //         // }
            //         Ok(())
            //     }
            //     None => {
            //         // Entry::Vacant(entry) => {
            //         self.deps.insert(node.clone(), new_deps);
            //         // entry.insert(new_deps);
            //         Ok(())
            //     }
            // }?;

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

    // pub fn remove_dependencies<'a>(&'a mut self, node: &'a I) {
    //     let remove: Vec<(_, _)> = self.dependencies(node).collect();
    //     for (_, dep) in remove {
    //         self.deps.remove(&dep);
    //         self.deps.remove(&dep);
    //     }
    // }

    pub fn set_state(&mut self, node: I, state: State) {
        self.states.insert(node, state);
    }

    pub fn update_ready_nodes<'a>(&'a mut self, node: &'a I) {
        assert!(!self.ready.remove(&node));
        self.deps.remove(&node);

        // let ready: Vec<I> = self
        //     .dependants(node)
        //     .filter_map(|(_, dependant)| match self.deps.get(&dependant) {
        //         Some(dependant_dependencies) => Some((dependant.clone(), dependant_dependencies)),
        //         None => None,
        //     })
        //     .filter_map(|(dependant, dependant_dependencies)| {
        //         if dependant_dependencies
        //             .iter()
        //             .map(|d| self.states.get(d))
        //             .all(|v| v == Some(&State::Success))
        //         {
        //             Some(dependant.clone())
        //         } else {
        //             None
        //         }
        //     })
        //     .collect();
        // self.ready.extend(ready);
        let mut ready = vec![];
        for (_, dependant) in self.dependants(node) {
            if let Some(dependant_dependencies) = self.deps.get(&dependant) {
                // do not add to ready queue unless all succeeded
                if dependant_dependencies
                    .iter()
                    .map(|d| self.states.get(d))
                    .all(|v| v == Some(&State::Success))
                {
                    ready.push(dependant.clone());
                }
            }
        }
        self.ready.extend(ready);
    }

    pub fn remove_dependants<'a>(&'a mut self, node: &'a I) {
        let remove: Vec<(_, _)> = self.recursive_dependants(node).collect();
        for (_, dep) in remove {
            self.dependants.remove(&dep);
            self.deps.remove(&dep);
            assert!(!self.ready.remove(&dep));
        }
        self.dependants.remove(&node);
        self.deps.remove(&node);
    }

    //     let mut seen = HashSet::<I>::new();
    //     let mut stack = Vec::<Node<I>>::new();

    //     // let next_nodes = self.remove(node)?;
    //     // if res.is_ok() {
    //     //     self.ready.extend(next_nodes);
    //     // }
    //     // self.ready.remove(&node);
    //     // self.deps.remove(&node);
    //     // replace only
    //     // let empty = HashSet::<I>::new();
    //     // let dependants = self.dependants.get(node).unwrap_or(&empty);
    //     // let dependants = self.dependants.get(node).unwrap_or(&empty);
    //     if let Some(dependants) = self.dependants.get(node) {
    //         stack.extend(dependants);
    //     }

    //     while let Some(dependant) = stack.pop() {
    //         match res {
    //             // if task failed, remove dependant
    //             Err(_) => {
    //                 // callback here
    //                 self.deps.remove(&dependant);
    //             }
    //             Ok(_) => {
    //                 // check if dependant is ready
    //                 if let Some(dependant_dependencies) = self.deps.get_mut(&dependant) {
    //                     dependant_dependencies.insert(node, State::Success);
    //                     if dependant_dependencies.values().all(|v| v == State::Success) {
    //                         self.ready.extend(dependant.clone());
    //                     }
    //                 }
    //             }
    //         }
    //         // match self.deps.get_mut(&dependant) {
    //         //     Some(dependant_dependencies) => {
    //         //         dependant_dependencies.remove(node);
    //         //         if dependant_dependencies.is_empty() {
    //         //             Some(dependant.clone())
    //         //         } else {
    //         //             None
    //         //         }
    //         //     }
    //     }
    // }

    pub fn ready<'a>(&'a self) -> impl Iterator<Item = &'a I> + 'a {
        self.ready.iter()
    }

    // pub fn extend(&mut self, nodes: DAG<I>) -> Result<(), ScheduleError<I>> {
    pub fn schedule(&mut self, id: &I) -> Result<(), ScheduleError<I>> {
        self.set_state(id.clone(), State::Running);
        match self.ready.remove(id) {
            true => Ok(()),
            false => Err(ScheduleError::BadPolicy(id.clone())),
        }
    }

    // pub fn dependencies<'a>(&'a mut self, node: &'a I) -> Option<impl Iterator<Item = &'a I> + 'a> {
    //     // pub fn dependencies<Iter>(&self, node: &I) -> Iter
    //     // where
    //     //     Iter: Iterator<Item = &I>,
    //     //     // impl Iterator<Item = &I>
    //     // {
    //     // let test = ;
    //     // self.deps.entry(node.clone()).or_insert(HashSet::new());
    //     self.deps.get(node).map(|deps| deps.iter())
    //     // self.deps.get(node).unwrap().iter()
    //     // self.deps.get(node).unwrap().iter()
    //     // self.deps.get(node).iter().cloned().flatten()
    //     // cloned().iter().cloned().flatten()
    //     // self.deps.get(node).cloned().iter().cloned().flatten()
    //     // self.deps.get(node).as_deref().unwrap_or_default().iter()
    //     // let test = self.deps.get(node).cloned();
    //     // let test2 = test.iter().flatten();
    //     // test2.cloned()
    //     // .map(|deps| deps.as_slice())
    //     // .unwrap_or(&[])
    //     // .iter()
    //     // .iter().flat_map(|v| v.iter())
    //     // .unwrap_or_else(HashSet::new).iter()
    //     // match self.deps.get(node) {
    //     //     Some(deps) => deps.iter(), // .cloned(),
    //     //     None => std::iter::empty::<&I>(),
    //     // }
    //     // std::iter::empty::<&I>()
    // }

    // fn dependants(&mut self, node: &I) -> impl Iterator<Item = &I> {
    //     // Option<&HashSet<I>> {
    //     // let empty = HashSet::<I>::new();
    //     self.dependants
    //         .get(node)
    //         .unwrap_or(std::iter::empty())
    //         .iter() // .unwrap_or(&empty)
    // }

    // fn remove(&mut self, node: &I) -> Result<HashSet<I>, ScheduleError<I>> {
    //     self.ready.remove(&node);
    //     // self.deps.remove(&node);
    //     // replace only
    //     let empty = HashSet::<I>::new();
    //     let dependants = self.dependants.get(node).unwrap_or(&empty);

    //     let free_nodes: HashSet<_> = dependants
    //         .iter()
    //         .filter_map(|dependant| match self.deps.get_mut(&dependant) {
    //             Some(dependant_dependencies) => {
    //                 dependant_dependencies.remove(node);
    //                 if dependant_dependencies.is_empty() {
    //                     Some(dependant.clone())
    //                 } else {
    //                     None
    //                 }
    //             }
    //             None => None,
    //         })
    //         .collect();
    //     Ok(free_nodes)
    // }

    // pub fn dependants<O, E>(
    //     &mut self,
    //     node: &I,
    //     res: &Result<O, E>,
    // ) -> Result<(), ScheduleError<I>> {

    // pub fn completed<O, E>(
    //     &mut self,
    //     node: &I,
    //     res: &Result<O, E>,
    // ) -> Result<(), ScheduleError<I>> {
    //     self.ready.remove(&node);
    //     self.deps.remove(&node);
    //     // self.deps.insert(node.clone(), match res {
    //     // Err(_) => Node::Failed(
    //     // });

    //     // let mut seen = HashSet::<I>::new();
    //     // let mut stack = Vec::<Node<I>>::new();

    //     // let next_nodes = self.remove(node)?;
    //     // if res.is_ok() {
    //     //     self.ready.extend(next_nodes);
    //     // }
    //     // self.ready.remove(&node);
    //     // self.deps.remove(&node);
    //     // replace only
    //     // let empty = HashSet::<I>::new();
    //     // let dependants = self.dependants.get(node).unwrap_or(&empty);
    //     // let dependants = self.dependants.get(node).unwrap_or(&empty);

    //     // if let Some(dependants) = self.dependants.get(node) {
    //     //     stack.extend(dependants);
    //     // }

    //     // while let Some(dependant) = stack.pop() {
    //     //     match res {
    //     //         // if task failed, remove dependant
    //     //         Err(_) => {
    //     //             // callback here
    //     //             self.deps.remove(&dependant);
    //     //         }
    //     //         Ok(_) => {
    //     //             // check if dependant is ready
    //     //             if let Some(dependant_dependencies) = self.deps.get_mut(&dependant) {
    //     //                 dependant_dependencies.insert(node, State::Success);
    //     //                 if dependant_dependencies.values().all(|v| v == State::Success) {
    //     //                     self.ready.extend(dependant.clone());
    //     //                 }
    //     //             }
    //     //         }
    //     //     }
    //     //     // match self.deps.get_mut(&dependant) {
    //     //     //     Some(dependant_dependencies) => {
    //     //     //         dependant_dependencies.remove(node);
    //     //     //         if dependant_dependencies.is_empty() {
    //     //     //             Some(dependant.clone())
    //     //     //         } else {
    //     //     //             None
    //     //     //         }
    //     //     //     }
    //     // }

    //     // let free_nodes: HashSet<_> = dependants
    //     //     .iter()
    //     //     .filter_map(|dependant| match self.deps.get_mut(&dependant) {
    //     //         Some(dependant_dependencies) => {
    //     //             dependant_dependencies.remove(node);
    //     //             if dependant_dependencies.is_empty() {
    //     //                 Some(dependant.clone())
    //     //             } else {
    //     //                 None
    //     //             }
    //     //         }
    //     //         None => None,
    //     //     })
    //     //     .collect();
    //     // Ok(free_nodes)

    //     Ok(())
    // }

    // pub fn ready(&self) -> &HashSet<I> {
    //     &self.ready
    // }

    // pub fn ready(&self) -> &HashSet<I> {
    //     &self.ready
    // }
}
