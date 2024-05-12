use crate::{policy, schedule::Schedule, task, trace};

use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::Instant;

pub trait Executor<L> {
    fn ready(&self) -> Box<dyn Iterator<Item = task::Ref<L>> + '_>;
    fn running(&self) -> HashSet<task::Ref<L>>;
}

/// An executor for a task schedule
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct PolicyExecutor<P, L> {
    pub schedule: Schedule<L>,
    pub policy: P,
    pub trace: Arc<trace::Trace<usize>>,
    pub running: Arc<RwLock<HashSet<task::Ref<L>>>>,
    pub ready: Vec<task::Ref<L>>,
}

impl<P, L> Executor<L> for &mut PolicyExecutor<P, L> {
    /// Iterator over all ready tasks
    fn ready<'a>(&'a self) -> Box<dyn Iterator<Item = task::Ref<L>> + 'a> {
        Box::new(self.ready.iter().cloned())
    }

    /// Iterator over all running tasks
    #[allow(clippy::missing_panics_doc)]
    fn running(&self) -> HashSet<task::Ref<L>> {
        // safety: panics if the lock is already held by the current thread.
        self.running.read().unwrap().clone()
    }
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
            running: Arc::new(RwLock::new(HashSet::new())),
            trace: Arc::new(trace::Trace::new()),
            ready: Vec::new(),
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
            running: Arc::new(RwLock::new(HashSet::new())),
            trace: Arc::new(trace::Trace::new()),
            policy: policy::Fifo::default(),
            ready: Vec::new(),
        }
    }
}

// TODO: bounded executor

impl<P, L> PolicyExecutor<P, L>
where
    P: policy::Policy<L>,
    L: 'static,
{
    /// Runs the tasks in the graph
    pub async fn run(&mut self) {
        use futures::stream::{FuturesUnordered, StreamExt};
        use std::future::Future;

        type TaskFut<LL> = dyn Future<Output = task::Ref<LL>>;
        let mut tasks: FuturesUnordered<Pin<Box<TaskFut<L>>>> = FuturesUnordered::new();

        self.ready = self
            .schedule
            .dependencies
            .keys()
            .filter(|t| t.ready())
            .cloned()
            .collect();
        dbg!(&self.ready);

        loop {
            // check if we are done
            if tasks.is_empty() && self.ready.is_empty() {
                println!("we are done");
                break;
            }

            // start running ready tasks
            while let Some(p) = self.policy.arbitrate(&self) {
                self.ready.retain(|r| r != &p);

                let trace = self.trace.clone();
                let running = self.running.clone();

                // todo: how would this look if we put the tracing and running stuff
                // before and after when the complete in the scheduler loop?
                tasks.push(Box::pin(async move {
                    println!("running {:?}", &p);

                    // safety: panics if the lock is already held by the current thread.
                    running.write().unwrap().insert(p.clone());
                    trace.tasks.lock().await.insert(
                        p.index(),
                        trace::Task {
                            label: p.short_name(),
                            start: Some(Instant::now()),
                            end: None,
                        },
                    );
                    p.run().await;
                    if let Some(task) = trace.tasks.lock().await.get_mut(&p.index()) {
                        task.end = Some(Instant::now());
                    }
                    p
                }));
            }

            // wait for a task to complete
            if let Some(completed) = tasks.next().await {
                // todo: check for output or error
                println!("task {} completed: {:?}", &completed, completed.state());
                self.running.write().unwrap().remove(&completed);
                match completed.state() {
                    task::CompletionResult::Pending | task::CompletionResult::Running { .. } => {
                        unreachable!("completed task state is invalid");
                    }
                    task::CompletionResult::Failed(_err) => {
                        // fail fast
                        self.schedule.fail_dependants(&completed, true);
                    }
                    task::CompletionResult::Succeeded => {}
                }
                // assert!(matches!(State::Pending(_), &completed));

                if let Some(dependants) = &self.schedule.dependants.get(&completed) {
                    println!("dependants: {:?}", &dependants);
                    // extend the ready queue
                    self.ready.extend(dependants.iter().filter_map(|d| {
                        if d.ready() {
                            Some(d.clone())
                        } else {
                            None
                        }
                    }));
                }
            }
        }
    }
}
