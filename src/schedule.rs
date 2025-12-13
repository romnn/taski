use petgraph as pg;

use crate::{
    dag::{self, Dfs, DAG},
    dependency::Dependencies,
    task, trace,
};

use futures::Future;
use pg::graph::NodeIndex;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A scheduling error.
///
/// This covers preconditions that cause tasks to fail during scheduling.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("task dependency failed")]
    FailedDependency,
}

pub(crate) type Fut = Pin<Box<dyn Future<Output = (dag::Idx, trace::Task)>>>;

/// Trait representing a schedulable task node.
///
/// TaskNodes implement this trait.
/// We cannot just use the TaskNode by itself,
/// because we need to combine task nodes with different
/// generic parameters.
pub trait Schedulable<L> {
    /// Indicates if the schedulable task has succeeded.
    ///
    /// A task is succeeded if its output is available.
    fn succeeded(&self) -> bool;

    /// Fails the schedulable task.
    fn fail(&self, err: task::Error);

    /// The result state of the task after completion.
    fn state(&self) -> task::State;

    /// The current task formatted as an argument.
    ///
    /// If the task succeeded, this is equivalent to its output.
    fn as_argument(&self) -> String;

    /// Signature of the task with the arguments.
    fn signature(&self) -> String {
        let arguments: Vec<_> = self
            .dependencies()
            .iter()
            .map(|dep| dep.as_argument())
            .collect();
        format!("{}({})", self.name(), arguments.join(", "))
    }

    /// The creation time of the task.
    fn created_at(&self) -> Instant;

    /// The starting time of the task.
    ///
    /// If the task is still pending, `None` is returned.
    fn started_at(&self) -> Option<Instant>;

    /// The completion time of the task.
    ///
    /// If the task is still pending or running,
    /// `None` is returned.
    fn completed_at(&self) -> Option<Instant>;

    /// Unique index of the task node in the DAG graph
    fn index(&self) -> dag::Idx;

    /// Run the schedulable task.
    ///
    /// The outputs of the dependencies are used as inputs.
    ///
    /// Running the task does not require a mutable borrow,
    /// since we only swap out the internal state which is
    /// protected using interior mutability.
    ///
    /// A schedulable task can only run exactly once,
    /// since the inner task is consumed and only the output
    /// or error is kept.
    ///
    /// Users must not manually run the task before all
    /// dependencies have completed.
    fn run(&self) -> Option<Fut>;

    /// Returns the name of the schedulable task
    fn name(&self) -> &str;

    /// Returns the label of this task.
    fn label(&self) -> &L;

    /// Returns the color of this task for rendering.
    fn color(&self) -> &Option<crate::render::Rgba>;

    /// Returns the dependencies of the schedulable task
    fn dependencies(&self) -> Vec<Arc<dyn Schedulable<L>>>;

    /// Indicates if the schedulable task is ready for execution.
    ///
    /// A task is ready if all its dependencies succeeded.
    fn is_ready(&self) -> bool {
        self.dependencies().iter().all(|d| d.succeeded())
    }

    /// The running_time time of the task.
    ///
    /// If the task has not yet completed, `None` is returned.
    fn running_time(&self) -> Option<Duration> {
        match (self.started_at(), self.completed_at()) {
            (Some(s), Some(e)) => Some(e.duration_since(s)),
            _ => None,
        }
    }

    /// The queue time of the task.
    fn queue_time(&self) -> Duration {
        match self.started_at() {
            Some(s) => s.duration_since(self.created_at()),
            None => self.created_at().elapsed(),
        }
    }
}

impl<L> std::fmt::Debug for dyn Schedulable<L> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
    }
}

impl<L> std::fmt::Debug for dyn Schedulable<L> + Send + Sync + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
    }
}

impl<L> std::fmt::Display for dyn Schedulable<L> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
    }
}

impl<L> std::fmt::Display for dyn Schedulable<L> + Send + Sync + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
    }
}

impl<L> Hash for dyn Schedulable<L> + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index().hash(state);
    }
}

impl<L> Hash for dyn Schedulable<L> + Send + Sync + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index().hash(state);
    }
}

impl<L> PartialEq for dyn Schedulable<L> + '_ {
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(&self.index(), &other.index())
    }
}

impl<L> PartialEq for dyn Schedulable<L> + Send + Sync + '_ {
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(&self.index(), &other.index())
    }
}

impl<L> Eq for dyn Schedulable<L> + '_ {}

impl<L> Eq for dyn Schedulable<L> + Send + Sync + '_ {}

/// A task schedule based on a DAG of task nodes.
#[derive(Debug, Clone)]
pub struct Schedule<L> {
    pub dag: DAG<task::Ref<L>>,
}

impl<L> Default for Schedule<L> {
    fn default() -> Self {
        Self {
            dag: DAG::default(),
        }
    }
}

impl<L> Schedule<L> {
    /// Add a task to the graph.
    ///
    /// A common `label` type may optionally be given to allow for custom
    /// scheduling policies.
    ///
    /// Dependencies for the task must be references to tasks that
    /// have already been added to the schedule and match the arguments
    /// of the added task.
    pub fn add_node<I, O, T, D>(&mut self, task: T, deps: D, label: L) -> Arc<task::Node<I, O, L>>
    where
        T: task::Task<I, O> + Send + Sync + 'static,
        D: Dependencies<I, L> + Send + Sync + 'static,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Sync + 'static,
    {
        let index = dag::Idx::new(self.dag.node_count());
        let node = Arc::new(task::Node::new(task, deps, label, index));
        let node_index = self.dag.add_node(node.clone());
        assert_eq!(node_index, index);

        // add edges to dependencies
        for dep in node.dependencies() {
            self.dag.add_edge(dep.index(), node_index, ());
        }

        node
    }

    /// Add a async closure to the graph.
    ///
    /// Using closures rather than tasks (`Task1`, `Task2` etc.) is more convenient
    /// for smaller functions, but not as powerful.
    /// In comparison, implementing the `Task` family of traits allows using a
    /// custom `name` (rather than `std::fmt::Debug`) and `color` for rendering.
    ///
    /// A common `label` type may optionally be given to allow for custom
    /// scheduling policies.
    ///
    /// Dependencies for the task must be references to tasks that
    /// have already been added to the schedule and match the arguments
    /// of the added task.
    pub fn add_closure<C, I, O, D>(
        &mut self,
        closure: C,
        deps: D,
        label: L,
    ) -> Arc<task::Node<I, O, L>>
    where
        C: task::Closure<I, O> + Send + Sync + 'static,
        D: Dependencies<I, L> + Send + Sync + 'static,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Sync + 'static,
    {
        let index = dag::Idx::new(self.dag.node_count());
        let closure = Box::new(closure);
        let node = Arc::new(task::Node::closure(closure, deps, label, index));
        let node_index = self.dag.add_node(node.clone());
        assert_eq!(node_index, index);
        for dep in node.dependencies() {
            self.dag.add_edge(dep.index(), node_index, ());
        }
        node
    }

    /// Add an input value without any dependencies to be used by other tasks.
    ///
    /// A common `label` type may optionally be given to allow for custom
    /// scheduling policies.
    pub fn add_input<O>(&mut self, input: O, label: L) -> Arc<task::Node<(), O, L>>
    where
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Sync + 'static,
    {
        self.add_node(task::Input::from(input), (), label)
    }

    /// Marks all dependants as failed.
    ///
    /// Optionally also marks all dependencies as failed if they have
    /// no pending dependants.
    ///
    /// TODO: is this correct and sufficient?
    /// TODO: really test this...
    pub fn fail_dependants(&mut self, root: dag::Idx, dependencies: bool) {
        let mut queue = vec![root];
        let mut processed = HashSet::new();
        processed.insert(root);

        while let Some(idx) = queue.pop() {
            dbg!(&queue);
            dbg!(&processed);

            // fail all dependants
            let mut new_processed = vec![];

            let dependants = pg::visit::Reversed(&self.dag);

            // // check if we can get the neighbors
            // use pg::visit::IntoNeighborsDirected;
            //
            // self.dag
            //     .neighbors_directed(task.index().into(), Incoming);
            // dependants.neighbors_directed(task.index().into(), Incoming);

            // let dfs = Dfs::new(&dependants, task);
            // let dfs = Dfs::new(&dependants, task.index().into());
            let dfs = Dfs::new(&dependants, idx);
            // let dfs = dfs.filter(|dep: &task::Ref<L>| {
            let mut dfs = dfs.filter(|dep_idx: &NodeIndex<usize>| {
                let dep = &self.dag[*dep_idx];
                dep.state().is_pending() && !processed.contains(dep_idx)
            });

            while let Some(dep_idx) = dfs.next(&self.dag) {
                // todo: link to failed dependency
                // if processed.insert(dependant) {
                let dependant = &self.dag[dep_idx];
                // new_processed.push(dependant);

                dependant.fail(task::Error::new(Error::FailedDependency));
                // processed.insert(dependant);

                new_processed.push(dep_idx);
                queue.push(dep_idx);

                //
            }

            // let dependants = self
            //     .dependants_dag
            //     .traverse(task, None, |dep: &&task::Ref<L>| {
            //         matches!(dep.state(), task::CompletionResult::Pending)
            //             && !processed.contains(dep)
            //     });
            // for (_, dependant) in dependants {
            //     // todo: link to failed dependency
            //     // if processed.insert(dependant) {
            //     new_processed.push(dependant);
            //     dependant.fail(Arc::new(task::Error::FailedDependency));
            //     // processed.insert(dependant);
            //     queue.push(dependant);
            //     // }
            // }
            // fail all dependencies of task
            if dependencies {
                // fail all dependencies
                todo!();
                // let dependencies =
                //     self.dependency_dag
                //         .traverse(task, None, |dep: &&task::Ref<L>| {
                //             matches!(dep.state(), task::CompletionResult::Pending)
                //                 && !processed.contains(task)
                //         });
                // for (_, dependency) in dependencies {
                //     // fail dependency
                //     // if processed.insert(dependency) {
                //     dependency.fail(Arc::new(task::Error::FailedDependency));
                //     // processed.insert(dependency);
                //     queue.push(dependency);
                //     new_processed.push(dependency);
                //     // }
                // }
            }

            processed.extend(new_processed);
        }
    }

    /// Iterator over all ready tasks in the schedule.
    ///
    /// A task is ready if all of its dependencies have successfully completed,
    /// such that all inputs are available to the task.
    pub fn ready(&self) -> impl Iterator<Item = dag::Idx> + '_ {
        self.dag
            .node_indices()
            .filter(|idx| self.dag[*idx].is_ready())
    }

    /// Iterator over all running tasks in the schedule.
    ///
    /// A task is in the `Running` state if it has been scheduled and its
    /// task future is being awaited.
    pub fn running(&self) -> impl Iterator<Item = dag::Idx> + '_ {
        self.dag
            .node_indices()
            .filter(|idx| self.dag[*idx].state().is_running())
    }
}
