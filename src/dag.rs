//! Low-level DAG identifiers and helpers.
//!
//! Most users interact with these indirectly via [`crate::Schedule`]. In particular:
//! - [`Handle`] is the typed handle you get back from `Schedule::add_*`.
//! - [`TaskId`] is an opaque identifier used internally and by policies.
//!
//! The `'id` lifetime brands values to a single schedule via `generativity`, preventing
//! accidental cross-schedule mixing.

use petgraph as pg;
use std::marker::PhantomData;

/// A node index in the underlying graph implementation.
pub type Idx = pg::graph::NodeIndex<usize>;

/// The underlying DAG type used by this crate.
pub type DAG<N> = pg::stable_graph::StableDiGraph<N, (), usize>;

/// Opaque identifier for a task node within a specific schedule.
///
/// `TaskId` values are *schedule-branded*: a task id from one schedule cannot be used with
/// another schedule/execution.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId<'id> {
    schedule_id: u64,
    idx: Idx,
    _invariant: PhantomData<fn(&'id ()) -> &'id ()>,
}

/// A typed handle to a task output.
///
/// Handles are created by [`crate::Schedule`] and can be used to declare dependencies and to read
/// outputs from an [`crate::execution::Execution`].
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Handle<'id, O> {
    task_id: TaskId<'id>,
    _output: PhantomData<O>,
}

impl<O> Copy for Handle<'_, O> {}

impl<O> Clone for Handle<'_, O> {
    fn clone(&self) -> Self {
        *self
    }
}

impl TaskId<'_> {
    #[must_use]
    pub(crate) fn new(schedule_id: u64, idx: Idx) -> Self {
        Self {
            schedule_id,
            idx,
            _invariant: PhantomData,
        }
    }

    #[must_use]
    pub(crate) fn schedule_id(self) -> u64 {
        self.schedule_id
    }

    #[must_use]
    /// Returns the underlying node index.
    pub fn idx(self) -> Idx {
        self.idx
    }
}

impl<'id, O> Handle<'id, O> {
    #[must_use]
    pub(crate) fn new(task_id: TaskId<'id>) -> Self {
        Self {
            task_id,
            _output: PhantomData,
        }
    }

    #[must_use]
    /// Returns the branded task id for this handle.
    pub fn task_id(self) -> TaskId<'id> {
        self.task_id
    }

    /// Returns the underlying [`TaskId`].
    ///
    /// This is mainly useful for policies and diagnostics.
    #[must_use]
    pub fn task_id_ref(&self) -> TaskId<'id> {
        self.task_id
    }
}

/// Depth first search.
///
/// **Note:**
/// The algorithm may not behave correctly if nodes are
/// removed during iteration.
/// It may not necessarily visit added nodes or edges.
#[derive(Clone, Debug)]
pub struct Dfs<N, VM, F> {
    /// The stack of nodes to visit.
    pub stack: Vec<(usize, N)>,
    /// The map of discovered nodes.
    pub discovered: VM,
    /// Maximum depth of the traversal.
    pub max_depth: Option<usize>,
    /// Filter nodes to traverse.
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
    /// Filters nodes to traverse.
    pub fn filter(mut self, filter: F) -> Self
    where
        F: Fn(&N) -> bool,
    {
        self.filter = Some(filter);
        self
    }

    #[must_use]
    /// Sets a maximum traversal depth.
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
