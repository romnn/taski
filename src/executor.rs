use crate::{dag, execution::Execution, policy, schedule, schedule::Schedule, task, trace};

use std::pin::Pin;

/// A report of the execution of a task schedule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunReport {
    pub total_tasks: usize,
    pub succeeded_tasks: usize,
    pub failed_tasks: usize,
}

/// An error that can occur when running a task schedule
#[derive(thiserror::Error, Debug)]
pub enum RunError {
    /// The execution stalled with unfinished tasks
    #[error("execution stalled with {unfinished_tasks} unfinished tasks")]
    Stalled {
        unfinished_tasks: usize,
        pending_task_indices: Vec<usize>,
    },
}

/// An executor for a task schedule
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct PolicyExecutor<'id, P, L> {
    schedule_id: u64,
    schedule: Schedule<'id, L>,
    execution: Execution<'id>,
    policy: P,
    trace: trace::Trace<dag::TaskId<'id>>,
}

impl<'id, P, L> PolicyExecutor<'id, P, L>
where
    L: 'static,
{
    /// Creates a new executor with a custom policy.
    #[must_use]
    pub fn custom(schedule: Schedule<'id, L>, policy: P) -> Self {
        let schedule_id = schedule.schedule_id();
        let execution = Execution::new(schedule_id, schedule.dag.node_count());
        Self {
            schedule_id,
            schedule,
            execution,
            policy,
            trace: trace::Trace::new(),
        }
    }

    #[must_use]
    pub fn schedule(&self) -> &Schedule<'id, L> {
        &self.schedule
    }

    #[must_use]
    pub fn execution(&self) -> &Execution<'id> {
        &self.execution
    }

    pub fn execution_mut(&mut self) -> &mut Execution<'id> {
        &mut self.execution
    }

    #[must_use]
    pub fn trace(&self) -> &trace::Trace<dag::TaskId<'id>> {
        &self.trace
    }

    pub fn trace_mut(&mut self) -> &mut trace::Trace<dag::TaskId<'id>> {
        &mut self.trace
    }
}

impl<'id, L> PolicyExecutor<'id, policy::Fifo, L>
where
    L: 'static,
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
        }
    }

    #[must_use]
    pub fn max_concurrent(mut self, limit: Option<usize>) -> Self {
        self.policy.max_concurrent = limit;
        self
    }
}

impl<'id, L> PolicyExecutor<'id, policy::Priority, L>
where
    L: std::cmp::Ord + 'static,
{
    /// Creates a new executor with a priority policy.
    ///
    /// The priority is given by the task label.
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
        }
    }

    #[must_use]
    pub fn max_concurrent(mut self, limit: Option<usize>) -> Self {
        self.policy.max_concurrent = limit;
        self
    }
}

impl<'id, P, L> PolicyExecutor<'id, P, L>
where
    P: policy::Policy<'id, L>,
    L: 'static,
{
    /// Runs the tasks in the graph
    pub async fn run(&mut self) -> Result<RunReport, RunError> {
        use futures::stream::{FuturesUnordered, StreamExt};
        use std::future::Future;
        use std::time::Instant;

        let mut running_tasks: FuturesUnordered<
            Pin<
                Box<
                    dyn Future<
                            Output = (
                                dag::TaskId<'id>,
                                trace::Task,
                                Result<std::sync::Arc<dyn std::any::Any + Send + Sync>, task::Error>,
                            ),
                        > + Send
                        + 'id,
                >,
            >,
        > = FuturesUnordered::new();

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

                if let Some(task_fut) = task.run(&self.execution) {
                    running_tasks.push(task_fut);
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
                        self.policy
                            .on_task_finished(failed_task_id, task::State::Failed, &self.schedule);
                    }
                }
            }

            if self.execution.unfinished_count() == 0 {
                break;
            }

            let Some((task_id, traced, result)) = running_tasks.next().await else {
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

            self.policy.on_task_finished(task_id, state.clone(), &self.schedule);

            if state.did_fail() {
                let failed = self.schedule.fail_dependants(&mut self.execution, task_id);
                for failed_task_id in failed {
                    failed_tasks = failed_tasks.saturating_add(1);
                    self.policy
                        .on_task_finished(failed_task_id, task::State::Failed, &self.schedule);
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
