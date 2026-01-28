//! Scheduling policies.
//!
//! A [`Policy`] decides which ready task should run next. Policies receive events about task state
//! transitions and can maintain internal queues.
//!
//! This crate provides two basic policies:
//! - [`Fifo`]: executes tasks in FIFO order.
//! - [`Priority`]: executes tasks in descending label order (ties broken by readiness order).

use crate::{dag, execution::Execution, schedule::Schedule, task};

use std::collections::VecDeque;

/// Scheduling policy for [`crate::PolicyExecutor`].
///
/// A policy interacts with the executor in an event-driven way:
/// - it is reset at the start of each run
/// - tasks are reported as *ready* once their dependencies complete
/// - the policy selects the next task to run via [`Policy::next_task`]
pub trait Policy<'id, L> {
    /// Resets internal policy state for a new run.
    fn reset(&mut self);

    /// Called when a task becomes ready to run.
    fn on_task_ready(&mut self, task_id: dag::TaskId<'id>, schedule: &Schedule<'id, L>);

    /// Called when a task is marked as running.
    fn on_task_started(&mut self, task_id: dag::TaskId<'id>, schedule: &Schedule<'id, L>);

    /// Called when a task finishes (succeeded or failed).
    fn on_task_finished(
        &mut self,
        task_id: dag::TaskId<'id>,
        state: task::State,
        schedule: &Schedule<'id, L>,
    );

    /// Selects the next task to run.
    ///
    /// Returning `None` means no task should be started right now.
    fn next_task(
        &mut self,
        schedule: &Schedule<'id, L>,
        execution: &Execution<'id>,
    ) -> Option<dag::TaskId<'id>>;
}

/// FIFO scheduling policy.
///
/// Tasks are executed in the order they become ready.
#[derive(Debug, Default, Clone)]
pub struct Fifo {
    /// Optional concurrency limit enforced by the policy.
    pub max_concurrent: Option<usize>,
    ready: VecDeque<dag::Idx>,
}

impl Fifo {
    /// Constructs the default FIFO policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructs a FIFO policy with an optional concurrency limit.
    #[must_use]
    pub fn max_concurrent(limit: Option<usize>) -> Self {
        Self::from_limit(limit)
    }

    /// Constructs a FIFO policy with an optional concurrency limit.
    #[must_use]
    pub fn from_limit(limit: Option<usize>) -> Self {
        Self {
            max_concurrent: limit,
            ready: VecDeque::new(),
        }
    }
}

impl<'id, L: 'id> Policy<'id, L> for Fifo {
    fn reset(&mut self) {
        self.ready.clear();
    }

    fn on_task_ready(&mut self, task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {
        self.ready.push_back(task_id.idx());
    }

    fn on_task_started(&mut self, _task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {}

    fn on_task_finished(
        &mut self,
        _task_id: dag::TaskId<'id>,
        _state: task::State,
        _schedule: &Schedule<'id, L>,
    ) {
    }

    fn next_task(
        &mut self,
        schedule: &Schedule<'id, L>,
        execution: &Execution<'id>,
    ) -> Option<dag::TaskId<'id>> {
        if let Some(limit) = self.max_concurrent
            && execution.running_count() >= limit {
                return None;
            }

        while let Some(task_idx) = self.ready.pop_front() {
            let task_id = schedule.task_id(task_idx);
            if execution.state(task_id).is_pending() {
                return Some(task_id);
            }
        }
        None
    }
}

/// Priority scheduling policy.
///
/// Tasks are executed in descending label order (ties are broken by readiness order).
#[derive(Debug, Default, Clone)]
pub struct Priority {
    /// Optional concurrency limit enforced by the policy.
    pub max_concurrent: Option<usize>,
    ready: Vec<dag::Idx>,
}

impl Priority {
    /// Constructs the default priority policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Constructs a priority policy with an optional concurrency limit.
    #[must_use]
    pub fn max_concurrent(limit: Option<usize>) -> Self {
        Self::from_limit(limit)
    }

    /// Constructs a priority policy with an optional concurrency limit.
    #[must_use]
    pub fn from_limit(limit: Option<usize>) -> Self {
        Self {
            max_concurrent: limit,
            ready: Vec::new(),
        }
    }
}

impl<'id, L> Policy<'id, L> for Priority
where
    L: std::cmp::Ord + 'id,
{
    fn reset(&mut self) {
        self.ready.clear();
    }

    fn on_task_ready(&mut self, task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {
        self.ready.push(task_id.idx());
    }

    fn on_task_started(&mut self, _task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {}

    fn on_task_finished(
        &mut self,
        _task_id: dag::TaskId<'id>,
        _state: task::State,
        _schedule: &Schedule<'id, L>,
    ) {
    }

    fn next_task(
        &mut self,
        schedule: &Schedule<'id, L>,
        execution: &Execution<'id>,
    ) -> Option<dag::TaskId<'id>> {
        if let Some(limit) = self.max_concurrent
            && execution.running_count() >= limit {
                return None;
            }

        let mut best: Option<(usize, &L)> = None;
        for (idx, &task_idx) in self.ready.iter().enumerate() {
            let task_id = schedule.task_id(task_idx);
            if !execution.state(task_id).is_pending() {
                continue;
            }

            let label = schedule.task_label(task_id);
            match best {
                None => best = Some((idx, label)),
                Some((best_idx, best_label)) => {
                    if label > best_label || (label == best_label && idx > best_idx) {
                        best = Some((idx, label));
                    }
                }
            }
        }

        let (best_idx, _) = best?;
        Some(schedule.task_id(self.ready.remove(best_idx)))
    }
}
