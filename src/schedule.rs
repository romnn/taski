use petgraph as pg;

use crate::{
    dag::{self, DAG},
    dependency::Dependencies,
    execution::Execution,
    task, trace,
};

use futures::Future;
use generativity::Guard;
use std::any::Any;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_SCHEDULE_ID: AtomicU64 = AtomicU64::new(1);

/// A scheduling error.
///
/// This covers preconditions that cause tasks to fail during scheduling.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("task dependency failed")]
    FailedDependency,
}

/// A schedule construction error.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum BuildError {
    #[error("dependency belongs to a different schedule")]
    DifferentScheduleDependency,

    #[error("dependency is not contained in the schedule")]
    MissingDependency,
}

pub(crate) type Fut<'id> =
    Pin<
        Box<
            dyn Future<
                    Output = (
                        dag::TaskId<'id>,
                        trace::Task,
                        Result<Arc<dyn Any + Send + Sync>, task::Error>,
                    ),
                > + Send
                + 'id,
        >,
    >;

/// Trait representing a schedulable task node.
///
/// TaskNodes implement this trait.
/// We cannot just use the TaskNode by itself,
/// because we need to combine task nodes with different
/// generic parameters.
pub trait Schedulable<'id, L: 'id>: Send + Sync + 'id {
    /// Indicates if the schedulable task has succeeded.
    ///
    /// A task is succeeded if its output is available.
    fn succeeded(&self, execution: &Execution<'id>) -> bool;

    /// Fails the schedulable task.
    fn fail(&self, execution: &mut Execution<'id>, err: task::Error);

    /// The result state of the task after completion.
    fn state(&self, execution: &Execution<'id>) -> task::State;

    /// The current task formatted as an argument.
    ///
    /// If the task succeeded, this is equivalent to its output.
    fn as_argument(&self, execution: &Execution<'id>) -> String;

    /// Signature of the task with the arguments.
    fn signature(&self) -> String {
        let dependencies: Vec<_> = self
            .dependencies()
            .iter()
            .map(|task_id| task_id.idx().index())
            .collect();
        format!("{}({dependencies:?})", self.name())
    }

    /// The creation time of the task.
    fn created_at(&self) -> Instant;

    /// The starting time of the task.
    ///
    /// If the task is still pending, `None` is returned.
    fn started_at(&self, execution: &Execution<'id>) -> Option<Instant>;

    /// The completion time of the task.
    ///
    /// If the task is still pending or running,
    /// `None` is returned.
    fn completed_at(&self, execution: &Execution<'id>) -> Option<Instant>;

    /// Unique index of the task node in the DAG graph
    fn index(&self) -> dag::TaskId<'id>;

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
    fn run(&self, execution: &Execution<'id>) -> Option<Fut<'id>>;

    /// Returns the name of the schedulable task
    fn name(&self) -> &str;

    /// Returns the label of this task.
    fn label(&self) -> &L;

    /// Returns the color of this task for rendering.
    fn color(&self) -> &Option<crate::render::Rgba>;

    /// Returns the dependencies of the schedulable task
    fn dependencies(&self) -> &[dag::TaskId<'id>];

    /// Indicates if the schedulable task is ready for execution.
    ///
    /// A task is ready if all its dependencies succeeded.
    fn is_ready(&self, execution: &Execution<'id>) -> bool {
        self.state(execution).is_pending()
            && self
                .dependencies()
                .iter()
                .all(|&task_id| execution.state(task_id).did_succeed())
    }

    /// The running_time time of the task.
    ///
    /// If the task has not yet completed, `None` is returned.
    fn running_time(&self, execution: &Execution<'id>) -> Option<Duration> {
        match (
            self.started_at(execution),
            self.completed_at(execution),
        ) {
            (Some(s), Some(e)) => Some(e.duration_since(s)),
            _ => None,
        }
    }

    /// The queue time of the task.
    fn queue_time(&self, execution: &Execution<'id>) -> Duration {
        match self.started_at(execution) {
            Some(s) => s.duration_since(self.created_at()),
            None => self.created_at().elapsed(),
        }
    }
}

impl<'id, L: 'id> std::fmt::Debug for dyn Schedulable<'id, L> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
    }
}

impl<'id, L: 'id> std::fmt::Display for dyn Schedulable<'id, L> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
    }
}

impl<'id, L: 'id> Hash for dyn Schedulable<'id, L> + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index().hash(state);
    }
}

impl<'id, L: 'id> PartialEq for dyn Schedulable<'id, L> + '_ {
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(&self.index(), &other.index())
    }
}

impl<'id, L: 'id> Eq for dyn Schedulable<'id, L> + '_ {}

/// A task schedule based on a DAG of task nodes.
#[derive(Debug)]
pub struct Schedule<'id, L: 'id> {
    _guard: generativity::Id<'id>,
    schedule_id: u64,
    pub(crate) dag: DAG<task::Ref<'id, L>>,
}

impl<'id, L: 'id> Schedule<'id, L> {
    #[must_use]
    pub fn new(guard: Guard<'id>) -> Self {
        Self {
            _guard: guard.into(),
            schedule_id: NEXT_SCHEDULE_ID.fetch_add(1, Ordering::Relaxed),
            dag: DAG::default(),
        }
    }

    #[must_use]
    pub(crate) fn schedule_id(&self) -> u64 {
        self.schedule_id
    }

    #[must_use]
    pub(crate) fn task_id(&self, idx: dag::Idx) -> dag::TaskId<'id> {
        dag::TaskId::new(self.schedule_id, idx)
    }

    /// Add a task to the graph.
    ///
    /// A common `label` type may optionally be given to allow for custom
    /// scheduling policies.
    ///
    /// Dependencies for the task must be references to tasks that
    /// have already been added to the schedule and match the arguments
    /// of the added task.
    pub fn add_node<I, O, T, D>(
        &mut self,
        task: T,
        deps: D,
        label: L,
    ) -> Result<dag::Handle<'id, O>, BuildError>
    where
        T: task::Task<I, O> + Send + Sync + 'static,
        D: Dependencies<'id, I> + Send + Sync + 'id,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Send + Sync + 'static,
    {
        let index = dag::TaskId::new(self.schedule_id, dag::Idx::new(self.dag.node_count()));
        let dependency_task_ids = deps.task_ids();
        for &dep_task_id in dependency_task_ids.iter() {
            if dep_task_id.schedule_id() != self.schedule_id {
                return Err(BuildError::DifferentScheduleDependency);
            }
            if !self.dag.contains_node(dep_task_id.idx()) {
                return Err(BuildError::MissingDependency);
            }
        }

        let node = Arc::new(task::Node::new(task, deps, dependency_task_ids, label, index));
        let node_index = self.dag.add_node(node.clone());
        if node_index != index.idx() {
            log::error!("node index mismatch");
        }

        // add edges to dependencies
        for &dep_task_id in node.dependencies() {
            self.dag.add_edge(dep_task_id.idx(), node_index, ());
        }

        Ok(dag::Handle::new(index))
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
    ) -> Result<dag::Handle<'id, O>, BuildError>
    where
        C: task::Closure<I, O> + Send + Sync + 'static,
        D: Dependencies<'id, I> + Send + Sync + 'id,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Send + Sync + 'static,
    {
        let index = dag::TaskId::new(self.schedule_id, dag::Idx::new(self.dag.node_count()));
        let closure = Box::new(closure);
        let dependency_task_ids = deps.task_ids();
        for &dep_task_id in dependency_task_ids.iter() {
            if dep_task_id.schedule_id() != self.schedule_id {
                return Err(BuildError::DifferentScheduleDependency);
            }
            if !self.dag.contains_node(dep_task_id.idx()) {
                return Err(BuildError::MissingDependency);
            }
        }

        let node = Arc::new(task::Node::closure(
            closure,
            deps,
            dependency_task_ids,
            label,
            index,
        ));
        let node_index = self.dag.add_node(node.clone());
        if node_index != index.idx() {
            log::error!("node index mismatch");
        }
        for &dep_task_id in node.dependencies() {
            self.dag.add_edge(dep_task_id.idx(), node_index, ());
        }
        Ok(dag::Handle::new(index))
    }

    /// Add an input value without any dependencies to be used by other tasks.
    ///
    /// A common `label` type may optionally be given to allow for custom
    /// scheduling policies.
    pub fn add_input<O>(&mut self, input: O, label: L) -> dag::Handle<'id, O>
    where
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Send + Sync + 'static,
    {
        #[expect(clippy::expect_used, reason = "inputs cannot have dependencies")]
        self.add_node(task::Input::from(input), (), label)
            .expect("add input")
    }

    /// Marks all dependants as failed.
    ///
    /// Optionally also marks all dependencies as failed if they have
    /// no pending dependants.
    ///
    /// TODO: is this correct and sufficient?
    /// TODO: really test this...
    pub fn fail_dependants(
        &mut self,
        execution: &mut Execution<'id>,
        root: dag::TaskId<'id>,
    ) -> Vec<dag::TaskId<'id>> {
        let mut queue = vec![root];
        let mut processed = HashSet::new();
        processed.insert(root);

        let mut failed = Vec::new();

        while let Some(idx) = queue.pop() {
            for dependant_idx in self
                .dag
                .neighbors_directed(idx.idx(), pg::Direction::Outgoing)
            {
                let dependant_idx = dag::TaskId::new(self.schedule_id, dependant_idx);
                if !processed.insert(dependant_idx) {
                    continue;
                }

                let dependant = &self.dag[dependant_idx.idx()];
                if dependant.state(execution).is_pending() {
                    dependant.fail(execution, task::Error::new(Error::FailedDependency));
                    failed.push(dependant_idx);
                }

                queue.push(dependant_idx);
            }
        }

        failed
    }

    /// Iterator over all ready tasks in the schedule.
    ///
    /// A task is ready if all of its dependencies have successfully completed,
    /// such that all inputs are available to the task.
    pub fn ready<'a>(
        &'a self,
        execution: &'a Execution<'id>,
    ) -> impl Iterator<Item = dag::TaskId<'id>> + 'a {
        self.dag
            .node_indices()
            .filter(|idx| self.dag[*idx].is_ready(execution))
            .map(|idx| dag::TaskId::new(self.schedule_id, idx))
    }

    /// Iterator over all running tasks in the schedule.
    ///
    /// A task is in the `Running` state if it has been scheduled and its
    /// task future is being awaited.
    pub fn running<'a>(
        &'a self,
        execution: &'a Execution<'id>,
    ) -> impl Iterator<Item = dag::TaskId<'id>> + 'a {
        self.dag
            .node_indices()
            .filter(|idx| self.dag[*idx].state(execution).is_running())
            .map(|idx| dag::TaskId::new(self.schedule_id, idx))
    }

    #[must_use]
    pub fn task(&self, task_id: dag::TaskId<'id>) -> &task::Ref<'id, L> {
        &self.dag[task_id.idx()]
    }

    #[must_use]
    pub fn task_label(&self, task_id: dag::TaskId<'id>) -> &L {
        self.task(task_id).label()
    }

    #[must_use]
    pub fn task_state(&self, execution: &Execution<'id>, task_id: dag::TaskId<'id>) -> task::State {
        self.task(task_id).state(execution)
    }
}
