use crate::{dag, policy, schedule::Schedule, task, trace};

use petgraph as pg;
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::Instant;

// pub trait Executor<L> {
//     // fn ready(&self) -> Box<dyn Iterator<Item = task::Ref<L>> + '_>;
//     fn ready(&self) -> impl Iterator<Item = > + '_>;
//     fn running(&self) -> HashSet<task::Ref<L>>;
// }

/// An executor for a task schedule
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct PolicyExecutor<P, L> {
    pub schedule: Schedule<L>,
    pub policy: P,
    pub trace: trace::Trace<usize>,
    // pub trace: Arc<trace::Trace<usize>>,
    // required?
    // pub running: Arc<RwLock<HashSet<task::Ref<L>>>>,
    // pub ready: Vec<dag::Idx>,
    // pub ready: Vec<task::Ref<L>>,
}

// impl<P, L> Executor<L> for &mut PolicyExecutor<P, L> {
//     /// Iterator over all ready tasks
//     // fn ready<'a>(&'a self) -> Box<dyn Iterator<Item = task::Ref<L>> + 'a> {
//     //     Box::new(self.ready.iter().cloned())
//     // }
//
//     /// Iterator over all running tasks
//     #[allow(clippy::missing_panics_doc)]
//     fn running(&self) -> HashSet<task::Ref<L>> {
//         // safety: panics if the lock is already held by the current thread.
//         self.running.read().unwrap().clone()
//     }
// }

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
            // running: Arc::new(RwLock::new(HashSet::new())),
            trace: trace::Trace::new(),
            // trace: Arc::new(trace::Trace::new()),
            // ready: Vec::new(),
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
            // running: Arc::new(RwLock::new(HashSet::new())),
            trace: trace::Trace::new(),
            // trace: Arc::new(trace::Trace::new()),
            policy: policy::Fifo::default(),
            // ready: Vec::new(),
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

        // type TaskFut<LL> = dyn Future<Output = (task::Ref<LL>, >;
        // type TaskFut<LL> = dyn Future<Output = (dag::Idx, trace::Task)>;
        //
        type TaskFut = dyn Future<Output = (dag::Idx, trace::Task)>;
        type TaskFuts = FuturesUnordered<Pin<Box<TaskFut>>>;
        // type TaskFuts<L> = FuturesUnordered<Pin<Box<TaskFut<L>>>>;

        let mut running_tasks: TaskFuts = FuturesUnordered::new();
        // let mut running_tasks: TaskFuts<L> = FuturesUnordered::new();

        let mut ready: Vec<_> = self.schedule.ready().collect();
        // self.ready = self.schedule.ready().collect();
        // .dag
        // .keys()
        // .filter(|t| t.ready())
        // .cloned()
        // .collect();

        // self.ready = self
        //     .schedule
        //     .dependency_dag
        //     .keys()
        //     .filter(|t| t.ready())
        //     .cloned()
        //     .collect();

        // dbg!(&self.ready);

        loop {
            // check if we are done
            // if running_tasks.is_empty() && self.ready.is_empty() {
            // if running_tasks.is_empty() && self.schedule.ready().next().is_none() {

            if running_tasks.is_empty() && ready.is_empty() {
                println!("we are done");
                break;
            }

            // dbg!(self.schedule.ready().collect::<Vec<_>>());
            // dbg!(&ready);

            // start running ready tasks
            while let Some(idx) = self.policy.arbitrate(&ready, &self.schedule) {
                assert!(ready.contains(&idx));
                ready.retain(|r| r != &idx);

                // dbg!(idx);
                let task = Arc::clone(&self.schedule.dag[idx]);
                // let trace = self.trace.clone();

                println!("adding {:?}", &task);
                // let running = self.running.clone();

                // todo: how would this look if we put the tracing and running stuff
                // before and after when the complete in the scheduler loop?
                running_tasks.push(Box::pin(async move {
                    println!("running {:?}", &task);

                    // safety: panics if the lock is already held by the current thread.
                    // running.write().unwrap().insert(p.clone());

                    // TODO: we could remove arc and mutex from trace when we
                    // just set the values later in the executor and return the
                    // values here
                    // trace.tasks.lock().await.insert(
                    //     p.index(),
                    //     trace::Task {
                    //         label: p.short_name(),
                    //         start: Some(Instant::now()),
                    //         end: None,
                    //     },
                    // );
                    let start_time = Instant::now();
                    task.run().await;
                    let end_time = Instant::now();

                    // if let Some(task) = trace.tasks.lock().await.get_mut(&p.index()) {
                    //     task.end = Some(Instant::now());
                    // }
                    (
                        idx,
                        trace::Task {
                            label: task.name().to_string(),
                            start: Some(start_time),
                            end: Some(end_time),
                        },
                    )

                    //     },

                    // (task, (start_time, end_time))
                }));
            }

            // dbg!("checking for completed");

            // wait for a task to complete
            if let Some((idx, traced)) = running_tasks.next().await {
                // todo: check for output or error
                self.trace.tasks.insert(idx.index(), traced);
                // self.trace.tasks.lock().await.insert(idx.index(), traced);

                let completed = &self.schedule.dag[idx];
                println!(
                    "task {} completed with status: {:?}",
                    &completed,
                    completed.state()
                );
                // self.running.write().unwrap().remove(&completed);

                let _states = self
                    .schedule
                    .dag
                    .node_indices()
                    .map(|idx| {
                        let task = &self.schedule.dag[idx];
                        (task.to_string(), task.state())
                    })
                    .collect::<Vec<_>>();
                // dbg!(_states);

                match completed.state() {
                    task::State::Pending | task::State::Running { .. } => {
                        unreachable!("completed task state is invalid");
                    }
                    // task::CompletionResult::Failed(_err) => {
                    task::State::Failed => {
                        // fail fast
                        self.schedule.fail_dependants(idx, true);
                    }
                    task::State::Succeeded => {}
                }
                // assert!(matches!(State::Pending(_), &completed));

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

                // let dependants_vec = dbg!(dependants.clone().count());
                // dbg!(dependants.clone().collect::<Vec<_>>());

                let _dependants = dependants
                    .clone()
                    .map(|idx| self.schedule.dag[idx].to_string())
                    .collect::<Vec<_>>();
                // dbg!(_dependants);

                let ready_dependants = dependants.filter_map(|dep_idx| {
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
                    if dep.ready() {
                        Some(dep_idx)
                    } else {
                        None
                    }
                });

                // let _ready_dependants = ready_dependants
                //     .clone()
                //     .map(|idx| self.schedule.dag[idx].to_string())
                //     .collect::<Vec<_>>();

                // dbg!(_ready_dependants);

                // println!("dependants: {:?}", &dependants);
                // extend the ready queue
                ready.extend(ready_dependants);

                // let dependants = pg::visit::Reversed(&self.schedule.dag);
                // dependants.

                // dependants
                // if let Some(dependants) = &self.schedule.dependants.get(&completed) {
                //     println!("dependants: {:?}", &dependants);
                //     // extend the ready queue
                //     self.ready.extend(dependants.iter().filter_map(|d| {
                //         if d.ready() {
                //             Some(d.clone())
                //         } else {
                //             None
                //         }
                //     }));
                // }
            }
        }
    }
}
