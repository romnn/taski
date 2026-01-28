use crate::{dag, execution::Execution, policy, schedule, schedule::Schedule, task, trace};

use std::pin::Pin;

/// An executor for a task schedule
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct PolicyExecutor<'id, P, L> {
    pub schedule: Schedule<'id, L>,
    pub execution: Execution<'id>,
    pub policy: P,
    pub trace: trace::Trace<dag::TaskId<'id>>,
}

impl<'id, P, L> PolicyExecutor<'id, P, L>
where
    L: 'static,
{
    /// Creates a new executor with a custom policy.
    #[must_use]
    pub fn custom(schedule: Schedule<'id, L>, policy: P) -> Self {
        let execution = Execution::new(schedule.dag.node_count());
        Self {
            schedule,
            execution,
            policy,
            trace: trace::Trace::new(),
        }
    }
}

impl<'id, L> PolicyExecutor<'id, policy::Fifo, L>
where
    L: 'static,
{
    /// Creates a new executor with a FIFO policy.
    #[must_use]
    pub fn fifo(schedule: Schedule<'id, L>) -> Self {
        let execution = Execution::new(schedule.dag.node_count());
        Self {
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
        let execution = Execution::new(schedule.dag.node_count());
        Self {
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
    pub async fn run(&mut self) {
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
                    .on_task_ready(dag::TaskId::new(node_idx), &self.schedule);
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
                    self.policy
                        .on_task_finished(task_id, task::State::Failed, &self.schedule);
                    let failed = self.schedule.fail_dependants(&mut self.execution, task_id);
                    for failed_task_id in failed {
                        self.policy
                            .on_task_finished(failed_task_id, task::State::Failed, &self.schedule);
                    }
                }
            }

            if self.execution.unfinished_count() == 0 {
                break;
            }

            let Some((task_id, traced, result)) = running_tasks.next().await else {
                log::error!("no running tasks but schedule is unfinished");
                break;
            };

            self.trace.tasks.push((task_id, traced));

            let now = Instant::now();
            let state = match result {
                Ok(output) => {
                    self.execution.mark_succeeded(task_id, now, output);
                    task::State::Succeeded
                }
                Err(err) => {
                    self.execution.mark_failed(task_id, now, err);
                    task::State::Failed
                }
            };

            self.policy.on_task_finished(task_id, state.clone(), &self.schedule);

            if state.did_fail() {
                let failed = self.schedule.fail_dependants(&mut self.execution, task_id);
                for failed_task_id in failed {
                    self.policy
                        .on_task_finished(failed_task_id, task::State::Failed, &self.schedule);
                }
            }

            for dependant_idx in self
                .schedule
                .dag
                .neighbors_directed(task_id.idx(), petgraph::Direction::Outgoing)
            {
                let dependant_task_id = dag::TaskId::new(dependant_idx);
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
    }
}
