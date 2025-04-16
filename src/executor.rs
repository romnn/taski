use crate::{dag, policy, schedule::Schedule, task, trace};

use std::pin::Pin;

/// An executor for a task schedule
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct PolicyExecutor<P, L> {
    pub schedule: Schedule<L>,
    pub policy: P,
    pub trace: trace::Trace<dag::Idx>,
}

impl<P, L> PolicyExecutor<P, L>
where
    L: 'static,
{
    /// Creates a new executor with a custom policy.
    #[must_use]
    pub fn custom(schedule: Schedule<L>, policy: P) -> Self {
        Self {
            schedule,
            policy,
            trace: trace::Trace::new(),
        }
    }
}

impl<L> PolicyExecutor<policy::Fifo, L>
where
    L: 'static,
{
    /// Creates a new executor with a FIFO policy.
    #[must_use]
    pub fn fifo(schedule: Schedule<L>) -> Self {
        Self {
            schedule,
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

impl<L> PolicyExecutor<policy::Priority, L>
where
    L: std::cmp::Ord + 'static,
{
    /// Creates a new executor with a priority policy.
    ///
    /// The priority is given by the task label.
    #[must_use]
    pub fn priority(schedule: Schedule<L>) -> Self {
        Self {
            schedule,
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

impl<P, L> PolicyExecutor<P, L>
where
    P: policy::Policy<L>,
    L: 'static,
{
    /// Runs the tasks in the graph
    pub async fn run(&mut self) {
        use futures::stream::{FuturesUnordered, StreamExt};
        use std::future::Future;

        type TaskFut = dyn Future<Output = (dag::Idx, trace::Task)>;
        type TaskFuts = FuturesUnordered<Pin<Box<TaskFut>>>;

        let mut running_tasks: TaskFuts = FuturesUnordered::new();

        let mut ready: Vec<_> = self.schedule.ready().collect();

        loop {
            // check if we are done
            if running_tasks.is_empty() && ready.is_empty() {
                log::debug!("completed: no more tasks");
                break;
            }

            // start running ready tasks
            while let Some(idx) = self.policy.arbitrate(&ready, &self.schedule) {
                assert!(ready.contains(&idx));
                ready.retain(|r| r != &idx);

                let task = &self.schedule.dag[idx];
                log::debug!("adding {:?}", &task);

                if let Some(task_fut) = task.run() {
                    running_tasks.push(task_fut);
                }
            }

            // wait for a task to complete
            if let Some((idx, traced)) = running_tasks.next().await {
                self.trace.tasks.push((idx, traced));

                let completed = &self.schedule.dag[idx];
                log::debug!(
                    "{} completed with status: {:?}",
                    &completed,
                    completed.state()
                );

                match completed.state() {
                    task::State::Pending | task::State::Running => {
                        unreachable!("completed task state is invalid");
                    }
                    task::State::Failed => {
                        // fail fast
                        self.schedule.fail_dependants(idx, true);
                    }
                    task::State::Succeeded => {}
                }

                // use pg::{visit::EdgeRef, visit::IntoEdgeReferences};
                // for edge in self.schedule.dag.edge_references() {
                //     println!(
                //         "have edge from {} to {}",
                //         self.schedule.dag[edge.source()],
                //         self.schedule.dag[edge.target()]
                //     );
                // }

                let dependants = self
                    .schedule
                    .dag
                    .neighbors_directed(idx, petgraph::Direction::Outgoing);

                // let _dependants = dependants
                //     .clone()
                //     .map(|idx| self.schedule.dag[idx].to_string())
                //     .collect::<Vec<_>>();
                // dbg!(_dependants);

                let ready_dependants = dependants.filter(|&dep_idx| {
                    let dep = &self.schedule.dag[dep_idx];
                    // dbg!(dep);
                    // println!(
                    //     "dependant {} has dependencies: {:?}",
                    //     dep,
                    //     dep.dependencies()
                    //         .iter()
                    //         .map(|d| d.state())
                    //         .collect::<Vec<_>>()
                    // );
                    dep.is_ready()
                });

                // extend the ready queue
                ready.extend(ready_dependants);
            }
        }
    }
}
