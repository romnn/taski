#![allow(clippy::missing_panics_doc)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! Async task DAG execution.
//!
//! `taski` lets you build a directed acyclic graph (DAG) of tasks and then execute it with a
//! scheduling policy.
//!
//! ## Quickstart
//!
//! Build a schedule, run it, then read outputs through typed handles:
//!
//! ```
//! use taski::{make_guard, PolicyExecutor, Schedule, TaskResult};
//! use futures::executor;
//!
//! async fn add_one(v: i32) -> TaskResult<i32> {
//!     Ok(v + 1)
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     executor::block_on(async {
//!         make_guard!(guard);
//!         let mut schedule: Schedule<'_, ()> = Schedule::new(guard);
//!
//!         let input = schedule.add_input(1_i32, ());
//!         let output = schedule.add_closure(add_one, (input,), ())?;
//!
//!         let mut executor = PolicyExecutor::fifo(schedule);
//!         executor.run().await?;
//!
//!         assert_eq!(executor.execution().output_ref(output).copied(), Some(2));
//!         Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
//!     })?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! - `render`: enables SVG rendering of schedules and execution traces.

pub mod dag;
pub mod dependency;
pub mod execution;
pub mod executor;
pub mod policy;
pub mod render;
pub mod schedule;
pub mod task;
pub mod trace;

pub use crate::dependency::Dependencies;
pub use executor::PolicyExecutor;
pub use generativity::make_guard;
pub use policy::Policy;
pub use schedule::Schedule;

pub use dag::Handle;

pub use task::{Closure1, Closure2, Closure3, Closure4, Closure5, Closure6, Closure7, Closure8};
pub use task::{Error, Input as TaskInput, Ref as TaskRef, Result as TaskResult};
pub use task::{Task0, Task1, Task2, Task3, Task4, Task5, Task6, Task7, Task8};

 #[cfg(doctest)]
 mod compile_fail_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::eyre;
    use std::time::Duration;
    use tokio::time::sleep;

    #[cfg(feature = "render")]
    macro_rules! function_name {
        () => {{
            fn f() {}
            fn type_name_of<T>(_: T) -> &'static str {
                std::any::type_name::<T>()
            }
            let name = type_name_of(f);
            name.strip_suffix("::f").unwrap_or(name)
        }};
    }

    #[cfg(feature = "render")]
    macro_rules! test_result_file {
        ($suffix:expr) => {{
            let manifest_dir = std::path::PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));
            let function_name = function_name!();
            let function_name = function_name
                .strip_suffix("::{{closure}}")
                .unwrap_or(function_name);
            let test = format!("{}_{}", function_name, $suffix);
            manifest_dir.join("src/tests/").join(test)
        }};
    }

    macro_rules! render {
        ($executor:expr) => {{
            #[cfg(feature = "render")]
            if should_render() {
                let path = test_result_file!("graph.svg");
                println!("rendering graph to {}", path.display());
                $executor.schedule().render_to(path)?;

                let path = test_result_file!("trace.svg");
                println!("rendering trace to {}", path.display());
                $executor.trace().render_to(path)?;
            }
        }};
    }

    #[cfg(feature = "render")]
    fn should_render() -> bool {
        std::env::var("RENDER")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
            == "yes"
    }

    #[allow(unused)]
    fn assert_unpin<C, A, F>(c: &C)
    where
        C: FnOnce(A) -> F,
        F: Unpin,
    {
    }

    #[allow(unused)]
    fn assert_closure<C, I, O>(c: &C)
    where
        C: task::Closure<I, O>,
    {
    }

    static INIT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();

    pub fn init_test() -> eyre::Result<()> {
        let result = INIT.get_or_init(|| {
            if let Err(err) = env_logger::builder().is_test(true).try_init() {
                return Err(format!("{err:?}"));
            }

            if let Err(err) = color_eyre::install() {
                return Err(format!("{err:?}"));
            }

            Ok(())
        });

        match result {
            Ok(()) => Ok(()),
            Err(err) => Err(eyre::eyre!(err)),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn priority_scheduler() -> eyre::Result<()> {
        #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
        struct Label {
            priority: usize,
        }

        impl std::cmp::Ord for Label {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.priority.cmp(&other.priority)
            }
        }

        impl std::cmp::PartialOrd for Label {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(std::cmp::Ord::cmp(self, other))
            }
        }

        init_test()?;

        make_guard!(guard);
        let mut graph = Schedule::new(guard);

        let n1_p1 = graph.add_closure(|| async { Ok(()) }, (), Label { priority: 1 })?;
        let n2_p1 = graph.add_closure(|| async { Ok(()) }, (), Label { priority: 1 })?;
        let n3_p2 = graph.add_closure(|| async { Ok(()) }, (), Label { priority: 2 })?;
        let n4_p1 = graph.add_closure(|| async { Ok(()) }, (), Label { priority: 1 })?;
        let n5_p4 = graph.add_closure(|| async { Ok(()) }, (), Label { priority: 4 })?;
        let n6_p3 = graph.add_closure(|| async { Ok(()) }, (), Label { priority: 3 })?;

        let mut executor = PolicyExecutor::priority(graph).max_concurrent(Some(1));

        // run all tasks
        executor.run().await?;

        // get the trace
        executor.trace_mut().sort_chronologically();
        assert_eq!(executor.trace().max_concurrent(), 1);

        let running_order: Vec<_> = executor.trace().tasks.iter().map(|(t, _)| *t).collect();
        let expected_order = [
            // high priority
            n5_p4.task_id(),
            n6_p3.task_id(),
            n3_p2.task_id(),
            // same priority is executed in reverse order of insertion
            n4_p1.task_id(),
            n2_p1.task_id(),
            n1_p1.task_id(),
        ];

        assert_eq!(running_order.as_slice(), &expected_order);

        render!(&executor);
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn custom_scheduler() -> eyre::Result<()> {
        use std::collections::HashMap;
        use std::collections::VecDeque;

        #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
        enum Label {
            Download,
            Process,
        }

        #[derive(Clone, Debug)]
        struct CustomPolicy {
            limits: HashMap<Label, usize>,
            ready: VecDeque<dag::Idx>,
            running_by_label: HashMap<Label, usize>,
        }

        impl<'id> Policy<'id, Label> for CustomPolicy {
            fn reset(&mut self) {
                self.ready.clear();
                self.running_by_label.clear();
            }

            fn on_task_ready(&mut self, task_id: dag::TaskId<'id>, _schedule: &schedule::Schedule<'id, Label>) {
                self.ready.push_back(task_id.idx());
            }

            fn on_task_started(
                &mut self,
                task_id: dag::TaskId<'id>,
                schedule: &schedule::Schedule<'id, Label>,
            ) {
                let label = *schedule.task_label(task_id);
                *self.running_by_label.entry(label).or_insert(0) += 1;
            }

            fn on_task_finished(
                &mut self,
                task_id: dag::TaskId<'id>,
                _state: task::State,
                schedule: &schedule::Schedule<'id, Label>,
            ) {
                let label = *schedule.task_label(task_id);
                if let Some(running) = self.running_by_label.get_mut(&label) {
                    *running = running.saturating_sub(1);
                    if *running == 0 {
                        self.running_by_label.remove(&label);
                    }
                }
            }

            fn next_task(
                &mut self,
                schedule: &schedule::Schedule<'id, Label>,
                execution: &execution::Execution<'id>,
            ) -> Option<dag::TaskId<'id>> {
                let attempts = self.ready.len();
                for _ in 0..attempts {
                    let Some(task_idx) = self.ready.pop_front() else {
                        break;
                    };
                    let task_id = schedule.task_id(task_idx);
                    if !execution.state(task_id).is_pending() {
                        continue;
                    }

                    let label = *schedule.task_label(task_id);
                    let running = self.running_by_label.get(&label).copied().unwrap_or(0);
                    match self.limits.get(&label) {
                        Some(limit) if running < *limit => return Some(task_id),
                        None => return Some(task_id),
                        _ => {
                            self.ready.push_back(task_idx);
                        }
                    }
                }

                None
            }
        }

        #[derive(Debug, Clone)]
        struct Download {}

        #[async_trait::async_trait]
        impl Task0<usize> for Download {
            async fn run(self: Box<Self>) -> TaskResult<usize> {
                sleep(Duration::from_secs(1)).await;
                Ok(0)
            }
            fn name(&self) -> String {
                "Download".to_string()
            }
            #[cfg(feature = "render")]
            fn color(&self) -> Option<crate::render::Rgba> {
                Some(crate::render::Rgba::from_hue(0.0.into()))
            }
        }

        #[derive(Debug, Clone)]
        struct Process {}

        #[async_trait::async_trait]
        impl crate::task::Task1<usize, usize> for Process {
            async fn run(self: Box<Self>, _: usize) -> TaskResult<usize> {
                sleep(Duration::from_secs(1)).await;
                Ok(0)
            }
            fn name(&self) -> String {
                "Process".to_string()
            }
            #[cfg(feature = "render")]
            fn color(&self) -> Option<crate::render::Rgba> {
                Some(crate::render::Rgba::from_hue(180.0.into()))
            }
        }

        init_test()?;

        make_guard!(guard);
        let mut graph = Schedule::new(guard);

        let download = Download {};
        let d1 = graph.add_node(download.clone(), (), Label::Download)?;
        let d2 = graph.add_node(download.clone(), (), Label::Download)?;
        let d3 = graph.add_node(download.clone(), (), Label::Download)?;
        let d4 = graph.add_node(download.clone(), (), Label::Download)?;
        let d5 = graph.add_node(download.clone(), (), Label::Download)?;
        let d6 = graph.add_node(download.clone(), (), Label::Download)?;

        let process = Process {};
        let _p1 = graph.add_node(process.clone(), (d1,), Label::Process)?;
        let _p2 = graph.add_node(process.clone(), (d2,), Label::Process)?;
        let _p3 = graph.add_node(process.clone(), (d3,), Label::Process)?;
        let _p4 = graph.add_node(process.clone(), (d4,), Label::Process)?;
        let _p5 = graph.add_node(process.clone(), (d5,), Label::Process)?;
        let _p6 = graph.add_node(process.clone(), (d6,), Label::Process)?;

        let mut executor = PolicyExecutor::custom(
            graph,
            CustomPolicy {
                // max two concurrent downloads
                // max three concurrent processes
                limits: HashMap::from_iter([(Label::Download, 3), (Label::Process, 2)]),
                ready: VecDeque::new(),
                running_by_label: HashMap::new(),
            },
        );
        executor.run().await?;

        render!(&executor);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fifo_scheduler() -> eyre::Result<()> {
        #[derive(Clone, Debug)]
        struct Identity {}

        #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
        enum Label {
            Input,
            Identity,
            Combine,
        }

        #[async_trait::async_trait]
        impl crate::task::Task1<String, String> for Identity {
            async fn run(self: Box<Self>, input: String) -> TaskResult<String> {
                sleep(Duration::from_secs(1)).await;
                Ok(input)
            }
        }

        #[derive(Clone, Debug)]
        struct Combine {}

        #[async_trait::async_trait]
        impl crate::task::Task2<String, String, String> for Combine {
            async fn run(self: Box<Self>, a: String, b: String) -> TaskResult<String> {
                sleep(Duration::from_secs(1)).await;
                Ok(format!("{} {}", &a, &b))
            }
        }

        init_test()?;

        let combine = Combine {};
        let identity = Identity {};

        make_guard!(guard);
        let mut graph = Schedule::new(guard);

        let i0 = graph.add_input("Hello".to_string(), Label::Input);

        let n0 = graph.add_node(identity.clone(), (i0,), Label::Identity)?;
        let n1 = graph.add_node(identity.clone(), (n0,), Label::Identity)?;
        let n2 = graph.add_node(identity.clone(), (n0,), Label::Identity)?;
        let result_node = graph.add_node(combine.clone(), (n1, n2), Label::Combine)?;

        let mut executor = PolicyExecutor::fifo(graph);

        // run all tasks
        executor.run().await?;

        assert_eq!(executor.trace().max_concurrent(), 2);

        render!(&executor);

        // check that output value of the scheduler is correct
        let output = executor.execution().output_ref(result_node).cloned();
        assert_eq!(output.as_deref(), Some("Hello Hello"));

        Ok(())
    }
}
