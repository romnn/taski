//! Schedule execution.
//!
//! The main entry point is [`PolicyExecutor`], which runs a [`crate::Schedule`] using a
//! user-provided [`crate::policy::Policy`] (or one of the built-in policies in [`crate::policy`]).
//!
//! Execution is asynchronous: tasks are polled concurrently up to the configured concurrency
//! limit, and dependants are scheduled once their prerequisites have completed successfully.

use crate::{dag, execution::Execution, policy, schedule, schedule::Schedule, task, trace};

use std::sync::Arc;
use std::time::Duration;
use tracing::Instrument;

/// A report of a schedule execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunReport {
    /// Total number of tasks in the schedule.
    pub total_tasks: usize,

    /// Number of tasks that completed successfully.
    pub succeeded_tasks: usize,

    /// Number of tasks that failed (including dependants failed due to dependency failure).
    pub failed_tasks: usize,
}

/// An error that can occur when running a schedule.
#[derive(thiserror::Error, Debug)]
pub enum RunError {
    /// The execution stalled with unfinished tasks.
    ///
    /// This happens if there are still pending tasks, but the policy produces no runnable task
    /// ids and there are no running futures.
    #[error("execution stalled with {unfinished_tasks} unfinished tasks")]
    Stalled {
        /// Number of tasks that have not yet finished.
        unfinished_tasks: usize,

        /// Node indices that are still pending.
        pending_task_indices: Vec<usize>,
    },

    /// The execution timed out.
    #[error("execution timed out after {timeout:?} with {unfinished_tasks} unfinished tasks")]
    TimedOut {
        /// Configured timeout.
        timeout: Duration,

        /// Number of tasks that have not yet finished.
        unfinished_tasks: usize,

        /// Node indices that are still pending.
        pending_task_indices: Vec<usize>,

        /// Node indices that were running at timeout.
        running_task_indices: Vec<usize>,
    },
}

/// An error used for failing individual tasks due to timeouts.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum TimeoutError {
    #[error("task timed out after {timeout:?}")]
    TaskTimedOut { timeout: Duration },
}

/// Executes a [`Schedule`] using a scheduling policy.
///
/// Construct an executor using one of the convenience constructors:
/// - [`PolicyExecutor::fifo`]
/// - [`PolicyExecutor::priority`]
/// - [`PolicyExecutor::custom`]
#[allow(clippy::module_name_repetitions)]
pub struct PolicyExecutor<'id, P, L, TTrace = trace::DefaultTaskTraceLayer> {
    schedule_id: u64,
    schedule: Schedule<'id, L>,
    execution: Execution<'id>,
    policy: P,
    trace: trace::Trace<dag::TaskId<'id>>,
    timeout: Option<Duration>,
    task_trace: TTrace,
}

impl<P, L, TTrace> std::fmt::Debug for PolicyExecutor<'_, P, L, TTrace>
where
    P: std::fmt::Debug,
    L: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyExecutor")
            .field("schedule_id", &self.schedule_id)
            .field("policy", &self.policy)
            .field("trace", &self.trace)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl<'id, P, L> PolicyExecutor<'id, P, L>
where
    L: 'id,
{
    /// Creates a new executor with a custom policy.
    #[must_use]
    pub fn new(schedule: Schedule<'id, L>, policy: P) -> Self {
        let schedule_id = schedule.schedule_id();
        let execution = Execution::new(schedule_id, schedule.dag.node_count());
        Self {
            schedule_id,
            schedule,
            execution,
            policy,
            trace: trace::Trace::new(),
            timeout: None,
            task_trace: trace::DefaultTaskTraceLayer::new(),
        }
    }

    #[must_use]
    pub fn with_task_trace<TTrace>(self, task_trace: TTrace) -> PolicyExecutor<'id, P, L, TTrace> {
        PolicyExecutor {
            schedule_id: self.schedule_id,
            schedule: self.schedule,
            execution: self.execution,
            policy: self.policy,
            trace: self.trace,
            timeout: self.timeout,
            task_trace,
        }
    }

    #[must_use]
    pub fn with_trace<TTrace>(self, task_trace: TTrace) -> PolicyExecutor<'id, P, L, TTrace> {
        self.with_task_trace(task_trace)
    }

    /// Sets an optional timeout for the entire execution.
    #[must_use]
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets an optional timeout for an individual task.
    pub fn set_task_timeout(&mut self, task_id: dag::TaskId<'id>, timeout: Option<Duration>) {
        self.execution.set_task_timeout(task_id, timeout);
    }

    /// Sets an optional timeout for an individual task.
    pub fn set_task_timeout_handle<O: 'static>(
        &mut self,
        handle: dag::Handle<'id, O>,
        timeout: Option<Duration>,
    ) {
        self.execution.set_task_timeout_handle(handle, timeout);
    }

    /// Returns the underlying schedule.
    #[must_use]
    pub fn schedule(&self) -> &Schedule<'id, L> {
        &self.schedule
    }

    /// Returns the current execution state.
    #[must_use]
    pub fn execution(&self) -> &Execution<'id> {
        &self.execution
    }

    /// Returns a mutable reference to the execution state.
    ///
    /// This is mainly useful for advanced introspection and testing.
    pub fn execution_mut(&mut self) -> &mut Execution<'id> {
        &mut self.execution
    }

    /// Returns references to the schedule and execution state.
    ///
    /// This avoids borrow conflicts when a caller needs schedule metadata while also mutating the
    /// execution.
    #[must_use]
    pub fn schedule_and_execution_mut(&mut self) -> (&Schedule<'id, L>, &mut Execution<'id>) {
        (&self.schedule, &mut self.execution)
    }

    /// Returns the execution trace collected during the run.
    #[must_use]
    pub fn trace(&self) -> &trace::Trace<dag::TaskId<'id>> {
        &self.trace
    }

    /// Returns a mutable reference to the execution trace.
    pub fn trace_mut(&mut self) -> &mut trace::Trace<dag::TaskId<'id>> {
        &mut self.trace
    }
}

impl<'id, L> PolicyExecutor<'id, policy::Fifo, L>
where
    L: 'id,
{
    /// Creates a new executor with a FIFO policy.
    #[must_use]
    pub fn fifo(schedule: Schedule<'id, L>) -> Self {
        let schedule_id = schedule.schedule_id();
        let execution = Execution::new(schedule_id, schedule.dag.node_count());
        Self {
            schedule_id,
            schedule,
            execution,
            trace: trace::Trace::new(),
            policy: policy::Fifo::default(),
            timeout: None,
            task_trace: trace::DefaultTaskTraceLayer::new(),
        }
    }

    /// Sets a maximum number of concurrently running tasks.
    #[must_use]
    pub fn max_concurrent(mut self, limit: Option<usize>) -> Self {
        self.policy.max_concurrent = limit;
        self
    }
}

impl<'id, L> PolicyExecutor<'id, policy::Priority, L>
where
    L: std::cmp::Ord + 'id,
{
    /// Creates a new executor with a priority policy.
    ///
    /// The priority is given by the task metadata.
    #[must_use]
    pub fn priority(schedule: Schedule<'id, L>) -> Self {
        let schedule_id = schedule.schedule_id();
        let execution = Execution::new(schedule_id, schedule.dag.node_count());
        Self {
            schedule_id,
            schedule,
            execution,
            trace: trace::Trace::new(),
            policy: policy::Priority::default(),
            timeout: None,
            task_trace: trace::DefaultTaskTraceLayer::new(),
        }
    }

    /// Sets a maximum number of concurrently running tasks.
    #[must_use]
    pub fn max_concurrent(mut self, limit: Option<usize>) -> Self {
        self.policy.max_concurrent = limit;
        self
    }
}

impl<'id, P, L, TTrace> PolicyExecutor<'id, P, L, TTrace>
where
    P: policy::Policy<'id, L>,
    L: 'id,
    TTrace: trace::TaskTrace<'id, L>,
{
    /// Runs the tasks in the graph.
    ///
    /// # Errors
    /// - If the executor stalls (no runnable tasks and no running futures).
    /// - If a task panics.
    #[allow(clippy::too_many_lines)]
    pub async fn run(&mut self) -> Result<RunReport, RunError> {
        use futures::stream::{FuturesUnordered, StreamExt};
        use std::time::Instant;

        let mut running_tasks: FuturesUnordered<schedule::TaskOutputFut<'id>> =
            FuturesUnordered::new();

        let deadline = self.timeout.map(|timeout| Instant::now() + timeout);
        let mut running_task_ids: Vec<dag::TaskId<'id>> = Vec::new();

        let node_count = self.schedule.dag.node_count();
        let mut succeeded_tasks = 0usize;
        let mut failed_tasks = 0usize;
        let mut remaining_dependencies = vec![0usize; node_count];
        for node_idx in self.schedule.dag.node_indices() {
            let incoming = self
                .schedule
                .dag
                .neighbors_directed(node_idx, petgraph::Direction::Incoming)
                .count();
            remaining_dependencies[node_idx.index()] = incoming;
        }

        self.policy.reset();
        for node_idx in self.schedule.dag.node_indices() {
            if remaining_dependencies[node_idx.index()] == 0 {
                self.policy
                    .on_task_ready(dag::TaskId::new(self.schedule_id, node_idx), &self.schedule);
            }
        }

        while self.execution.unfinished_count() > 0 {
            while let Some(task_id) = self.policy.next_task(&self.schedule, &self.execution) {
                let task = &self.schedule.dag[task_id.idx()];
                log::debug!("adding {:?}", &task);

                self.execution.mark_running(task_id, Instant::now());
                self.policy.on_task_started(task_id, &self.schedule);

                running_task_ids.push(task_id);

                if let Some(task_fut) = task.run(&self.execution) {
                    let mut task_trace = self.task_trace.clone();
                    let task_timeout = self.execution.task_timeout(task_id);
                    let task_context = trace::TaskContext {
                        task_id,
                        task: Arc::clone(task),
                        timeout: task_timeout,
                    };

                    let span = task_trace.make_span(&task_context);
                    task_trace.on_start(&task_context, &span);

                    if let Some(task_timeout) = task_timeout {
                        let label = task.to_string();
                        #[cfg(feature = "render")]
                        let color = *task.color();
                        let start = self
                            .execution
                            .started_at(task_id)
                            .unwrap_or_else(Instant::now);
                        let timeout_fut = futures_timer::Delay::new(task_timeout);
                        let span_for_instrument = span.clone();
                        let task_fut: schedule::TaskOutputFut<'id> = Box::pin(
                            async move {
                                futures::pin_mut!(task_fut);
                                futures::pin_mut!(timeout_fut);

                                match futures::future::select(task_fut, timeout_fut).await {
                                    futures::future::Either::Left((output, _)) => {
                                        match &output.2 {
                                            Ok(_) => task_trace.on_result(
                                                &task_context,
                                                trace::TaskResultKind::Succeeded,
                                                &span,
                                            ),
                                            Err(err) => task_trace.on_result(
                                                &task_context,
                                                trace::TaskResultKind::Failed { err },
                                                &span,
                                            ),
                                        }
                                        output
                                    }
                                    futures::future::Either::Right(((), _)) => {
                                        let end = Instant::now();
                                        let traced = crate::trace::Task {
                                            label,
                                            #[cfg(feature = "render")]
                                            color,
                                            start,
                                            end,
                                        };
                                        let output = (
                                            task_id,
                                            traced,
                                            Err(task::Error::new(TimeoutError::TaskTimedOut {
                                                timeout: task_timeout,
                                            })),
                                        );

                                        task_trace.on_result(
                                            &task_context,
                                            trace::TaskResultKind::TimedOut {
                                                timeout: task_timeout,
                                            },
                                            &span,
                                        );

                                        output
                                    }
                                }
                            }
                            .instrument(span_for_instrument),
                        );
                        running_tasks.push(task_fut);
                    } else {
                        let span_for_instrument = span.clone();
                        let task_fut = Box::pin(
                            async move {
                                let output = task_fut.await;
                                match &output.2 {
                                    Ok(_) => task_trace.on_result(
                                        &task_context,
                                        trace::TaskResultKind::Succeeded,
                                        &span,
                                    ),
                                    Err(err) => task_trace.on_result(
                                        &task_context,
                                        trace::TaskResultKind::Failed { err },
                                        &span,
                                    ),
                                }
                                output
                            }
                            .instrument(span_for_instrument),
                        );

                        running_tasks.push(task_fut);
                    }
                } else {
                    task.fail(
                        &mut self.execution,
                        task::Error::new(schedule::Error::FailedDependency),
                    );
                    failed_tasks = failed_tasks.saturating_add(1);
                    self.policy
                        .on_task_finished(task_id, task::State::Failed, &self.schedule);
                    let failed = self.schedule.fail_dependants(&mut self.execution, task_id);
                    for failed_task_id in failed {
                        failed_tasks = failed_tasks.saturating_add(1);
                        self.policy.on_task_finished(
                            failed_task_id,
                            task::State::Failed,
                            &self.schedule,
                        );
                    }
                }
            }

            if self.execution.unfinished_count() == 0 {
                break;
            }

            let next_output = if let Some(deadline) = deadline {
                let now = Instant::now();
                if now >= deadline {
                    let pending_task_indices: Vec<_> = self
                        .schedule
                        .dag
                        .node_indices()
                        .filter_map(|idx| {
                            let task_id = dag::TaskId::new(self.schedule_id, idx);
                            if self.execution.state(task_id).is_pending() {
                                Some(idx.index())
                            } else {
                                None
                            }
                        })
                        .collect();

                    let running_task_indices: Vec<_> = running_task_ids
                        .iter()
                        .filter(|&&task_id| self.execution.state(task_id).is_running())
                        .map(|task_id| task_id.idx().index())
                        .collect();

                    return Err(RunError::TimedOut {
                        timeout: self.timeout.unwrap_or_default(),
                        unfinished_tasks: self.execution.unfinished_count(),
                        pending_task_indices,
                        running_task_indices,
                    });
                }

                let remaining = deadline.saturating_duration_since(now);
                let timeout_fut = futures_timer::Delay::new(remaining);
                let next_task_fut = running_tasks.next();
                futures::pin_mut!(timeout_fut);
                futures::pin_mut!(next_task_fut);
                match futures::future::select(next_task_fut, timeout_fut).await {
                    futures::future::Either::Left((output, _)) => output,
                    futures::future::Either::Right(((), _)) => {
                        let pending_task_indices: Vec<_> = self
                            .schedule
                            .dag
                            .node_indices()
                            .filter_map(|idx| {
                                let task_id = dag::TaskId::new(self.schedule_id, idx);
                                if self.execution.state(task_id).is_pending() {
                                    Some(idx.index())
                                } else {
                                    None
                                }
                            })
                            .collect();

                        let running_task_indices: Vec<_> = running_task_ids
                            .iter()
                            .filter(|&&task_id| self.execution.state(task_id).is_running())
                            .map(|task_id| task_id.idx().index())
                            .collect();

                        return Err(RunError::TimedOut {
                            timeout: self.timeout.unwrap_or_default(),
                            unfinished_tasks: self.execution.unfinished_count(),
                            pending_task_indices,
                            running_task_indices,
                        });
                    }
                }
            } else {
                running_tasks.next().await
            };

            let Some((task_id, traced, result)) = next_output else {
                let pending_task_indices: Vec<_> = self
                    .schedule
                    .dag
                    .node_indices()
                    .filter_map(|idx| {
                        let task_id = dag::TaskId::new(self.schedule_id, idx);
                        if self.execution.state(task_id).is_pending() {
                            Some(idx.index())
                        } else {
                            None
                        }
                    })
                    .collect();

                return Err(RunError::Stalled {
                    unfinished_tasks: self.execution.unfinished_count(),
                    pending_task_indices,
                });
            };

            running_task_ids.retain(|&id| id != task_id);

            self.trace.tasks.push((task_id, traced));

            let now = Instant::now();
            let state = match result {
                Ok(output) => {
                    self.execution.mark_succeeded(task_id, now, output);
                    succeeded_tasks = succeeded_tasks.saturating_add(1);
                    task::State::Succeeded
                }
                Err(err) => {
                    self.execution.mark_failed(task_id, now, err);
                    failed_tasks = failed_tasks.saturating_add(1);
                    task::State::Failed
                }
            };

            self.policy
                .on_task_finished(task_id, state.clone(), &self.schedule);

            if state.did_fail() {
                let failed = self.schedule.fail_dependants(&mut self.execution, task_id);
                for failed_task_id in failed {
                    failed_tasks = failed_tasks.saturating_add(1);
                    self.policy.on_task_finished(
                        failed_task_id,
                        task::State::Failed,
                        &self.schedule,
                    );
                }
            }

            for dependant_idx in self
                .schedule
                .dag
                .neighbors_directed(task_id.idx(), petgraph::Direction::Outgoing)
            {
                let dependant_task_id = dag::TaskId::new(self.schedule_id, dependant_idx);
                if !self.execution.state(dependant_task_id).is_pending() {
                    continue;
                }

                let dependant_idx_usize = dependant_idx.index();
                let Some(remaining) = remaining_dependencies.get_mut(dependant_idx_usize) else {
                    continue;
                };

                if *remaining == 0 {
                    continue;
                }
                *remaining -= 1;
                if *remaining == 0 {
                    self.policy.on_task_ready(dependant_task_id, &self.schedule);
                }
            }
        }

        Ok(RunReport {
            total_tasks: node_count,
            succeeded_tasks,
            failed_tasks,
        })
    }
}
