//! Execution state for a running schedule.
//!
//! [`Execution`] tracks per-task state, timings, outputs, and errors while a schedule is being
//! executed by a [`crate::PolicyExecutor`].
//!
//! Access to task outputs is type-safe via [`dag::Handle`]. Internally, outputs are stored as
//! `Arc<dyn Any + Send + Sync>` and downcast on demand.

use crate::{dag, task};

use std::any::Any;
use std::time::{Duration, Instant};

/// Per-task execution data.
#[derive(Debug)]
pub struct TaskExecution {
    /// Current task state.
    pub state: task::State,
    /// When the task started running (if ever).
    pub started_at: Option<Instant>,
    /// When the task completed (if ever).
    pub completed_at: Option<Instant>,
    /// Output produced by a successful task.
    pub output: Option<Box<dyn Any + Send>>,
    /// Error produced by a failed task.
    pub error: Option<task::Error>,
}

impl Default for TaskExecution {
    fn default() -> Self {
        Self {
            state: task::State::Pending,
            started_at: None,
            completed_at: None,
            output: None,
            error: None,
        }
    }
}

#[derive(Debug, Default)]
/// Execution state for a branded schedule.
///
/// An `Execution` is created by a [`crate::PolicyExecutor`] and updated as tasks transition through
/// [`task::State`]s.
///
/// The `'id` lifetime ensures that task ids/handles belong to the same schedule.
pub struct Execution<'id> {
    schedule_id: u64,
    tasks: Vec<TaskExecution>,
    task_timeouts: Vec<Option<Duration>>,
    running_count: usize,
    unfinished_count: usize,
    _phantom: std::marker::PhantomData<fn(&'id ()) -> &'id ()>,
}

impl<'id> Execution<'id> {
    #[must_use]
    pub(crate) fn new(schedule_id: u64, task_count: usize) -> Self {
        Self {
            schedule_id,
            tasks: (0..task_count).map(|_| TaskExecution::default()).collect(),
            task_timeouts: (0..task_count).map(|_| None).collect(),
            running_count: 0,
            unfinished_count: task_count,
            _phantom: std::marker::PhantomData,
        }
    }

    pub(crate) fn task_timeout(&self, task_id: dag::TaskId<'id>) -> Option<Duration> {
        if !self.validate_task_id(task_id) {
            return None;
        }

        self.task_timeouts
            .get(task_id.idx().index())
            .copied()
            .flatten()
    }

    pub fn set_task_timeout(&mut self, task_id: dag::TaskId<'id>, timeout: Option<Duration>) {
        if !self.validate_task_id(task_id) {
            return;
        }

        let Some(slot) = self.task_timeouts.get_mut(task_id.idx().index()) else {
            log::error!("invalid task id");
            return;
        };
        *slot = timeout;
    }

    pub fn set_task_timeout_handle<O: 'static>(
        &mut self,
        handle: dag::Handle<'id, O>,
        timeout: Option<Duration>,
    ) {
        self.set_task_timeout(handle.task_id(), timeout);
    }

    fn validate_task_id(&self, task_id: dag::TaskId<'id>) -> bool {
        if task_id.schedule_id() != self.schedule_id {
            log::error!("task id belongs to a different schedule");
            return false;
        }
        true
    }

    pub(crate) fn mark_running(&mut self, task_id: dag::TaskId<'id>, now: Instant) {
        if !self.validate_task_id(task_id) {
            return;
        }
        let Some(task) = self.tasks.get_mut(task_id.idx().index()) else {
            log::error!("invalid task id");
            return;
        };

        if task.state.is_running() {
            return;
        }

        if task.state.is_pending() {
            self.running_count = self.running_count.saturating_add(1);
        }

        task.started_at.get_or_insert(now);
        task.state = task::State::Running;
    }

    pub(crate) fn mark_succeeded(
        &mut self,
        task_id: dag::TaskId<'id>,
        now: Instant,
        output: Box<dyn Any + Send>,
    ) {
        if !self.validate_task_id(task_id) {
            return;
        }
        let Some(task) = self.tasks.get_mut(task_id.idx().index()) else {
            log::error!("invalid task id");
            return;
        };

        if task.state.is_pending() || task.state.is_running() {
            if task.state.is_running() {
                self.running_count = self.running_count.saturating_sub(1);
            }
            self.unfinished_count = self.unfinished_count.saturating_sub(1);
        }

        task.completed_at.get_or_insert(now);
        task.output = Some(output);
        task.error = None;
        task.state = task::State::Succeeded;
    }

    pub(crate) fn mark_failed(
        &mut self,
        task_id: dag::TaskId<'id>,
        now: Instant,
        err: task::Error,
    ) {
        if !self.validate_task_id(task_id) {
            return;
        }
        let Some(task) = self.tasks.get_mut(task_id.idx().index()) else {
            log::error!("invalid task id");
            return;
        };

        if task.state.is_pending() || task.state.is_running() {
            if task.state.is_running() {
                self.running_count = self.running_count.saturating_sub(1);
            }
            self.unfinished_count = self.unfinished_count.saturating_sub(1);
        }

        task.completed_at.get_or_insert(now);
        task.output = None;
        task.error = Some(err);
        task.state = task::State::Failed;
    }

    #[must_use]
    /// Returns the number of tasks currently in the `Running` state.
    pub fn running_count(&self) -> usize {
        self.running_count
    }

    #[must_use]
    /// Returns the number of tasks that have not yet completed.
    pub fn unfinished_count(&self) -> usize {
        self.unfinished_count
    }

    /// Returns the current [`task::State`] of the given task.
    ///
    /// If the task id does not belong to this execution, `Pending` is returned.
    #[must_use]
    pub fn state(&self, task_id: dag::TaskId<'id>) -> task::State {
        if task_id.schedule_id() != self.schedule_id {
            return task::State::Pending;
        }
        self.tasks
            .get(task_id.idx().index())
            .map_or(task::State::Pending, |t| t.state.clone())
    }

    #[must_use]
    pub(crate) fn output_ref_task_id<O: 'static>(&self, task_id: dag::TaskId<'id>) -> Option<&O> {
        if !self.validate_task_id(task_id) {
            return None;
        }
        let task = self.tasks.get(task_id.idx().index())?;
        let output = task.output.as_ref()?;
        output.downcast_ref::<O>()
    }

    #[must_use]
    pub(crate) fn take_output_task_id<O: 'static>(
        &mut self,
        task_id: dag::TaskId<'id>,
    ) -> Option<O> {
        if !self.validate_task_id(task_id) {
            return None;
        }
        let task = self.tasks.get_mut(task_id.idx().index())?;
        let output = task.output.take()?;
        match output.downcast::<O>() {
            Ok(output) => Some(*output),
            Err(output) => {
                task.output = Some(output);
                None
            }
        }
    }

    /// Returns a typed reference to a task output.
    ///
    /// Returns `None` if the task has not completed successfully or if the type does not match.
    #[must_use]
    pub fn output_ref<O: 'static>(&self, handle: dag::Handle<'id, O>) -> Option<&O> {
        self.output_ref_task_id::<O>(handle.task_id())
    }

    #[must_use]
    pub fn take_output<O: 'static>(&mut self, handle: dag::Handle<'id, O>) -> Option<O> {
        self.take_output_task_id::<O>(handle.task_id())
    }

    #[must_use]
    pub fn take_output_dyn(&mut self, task_id: dag::TaskId<'id>) -> Option<Box<dyn Any + Send>> {
        if !self.validate_task_id(task_id) {
            return None;
        }
        let task = self.tasks.get_mut(task_id.idx().index())?;
        task.output.take()
    }

    /// Returns the time the task started, if known.
    #[must_use]
    pub fn started_at(&self, task_id: dag::TaskId<'id>) -> Option<Instant> {
        if !self.validate_task_id(task_id) {
            return None;
        }
        self.tasks.get(task_id.idx().index())?.started_at
    }

    /// Returns the time the task completed, if known.
    #[must_use]
    pub fn completed_at(&self, task_id: dag::TaskId<'id>) -> Option<Instant> {
        if !self.validate_task_id(task_id) {
            return None;
        }
        self.tasks.get(task_id.idx().index())?.completed_at
    }
}
