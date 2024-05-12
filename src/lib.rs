#![allow(clippy::missing_panics_doc)]

pub mod dag;
pub mod dependency;
pub mod dfs;
pub mod executor;
pub mod policy;
pub mod schedule;
pub mod task;
pub mod trace;

pub use crate::dependency::Dependency;
pub use executor::PolicyExecutor;
pub use policy::Policy;
pub use schedule::Schedule;
pub use task::{Input as TaskInput, Ref as TaskRef, Result as TaskResult, Task1, Task2};

// use async_trait::async_trait;
// use futures::stream::StreamExt;
//
// use std::collections::{HashMap, HashSet};
// use std::hash::{Hash, Hasher};
// use std::path::Path;
// use std::pin::Pin;
// use std::sync::Arc;
// use std::sync::RwLock;
// use std::time::{Duration, Instant};

//
//
// #[derive(Debug)]
// enum State<I, O> {
//     /// Task is pending and waiting to be run
//     Pending {
//         created: Instant,
//         task: Box<dyn Task<I, O> + Send + Sync + 'static>,
//     },
//     /// Task is running
//     Running { created: Instant, started: Instant },
//     /// Task succeeded with the desired output
//     Succeeded {
//         created: Instant,
//         started: Instant,
//         completed: Instant,
//         output: O,
//     },
//     /// Task failed with an error
//     Failed {
//         created: Instant,
//         started: Instant,
//         completed: Instant,
//         error: Arc<dyn std::error::Error + Send + Sync + 'static>,
//     },
// }

// #[derive(Debug, Clone)]
// pub enum CompletionResult {
//     /// Task is pending
//     Pending { created: Instant },
//     /// Task is running
//     Running { created: Instant, started: Instant },
//     /// Task succeeded
//     Succeeded {
//         created: Instant,
//         started: Instant,
//         completed: Instant,
//     },
//     /// Task failed with an error
//     Failed {
//         created: Instant,
//         started: Instant,
//         completed: Instant,
//         error: Arc<dyn std::error::Error + Send + Sync + 'static>,
//     },
// }

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::eyre;

    #[cfg(feature = "render")]
    macro_rules! function_name {
        () => {{
            fn f() {}
            fn type_name_of<T>(_: T) -> &'static str {
                std::any::type_name::<T>()
            }
            let name = type_name_of(f);
            &name[..name.len() - 3]
        }};
    }

    #[cfg(feature = "render")]
    macro_rules! test_result_file {
        ($suffix:expr) => {{
            let manifest_path = std::path::PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));
            let function_name = function_name!();
            let function_name = function_name
                .strip_suffix("::{{closure}}")
                .unwrap_or(function_name);
            let test = format!("{}_{}", function_name, $suffix);
            manifest_path.join("src/tests/").join(test)
        }};
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_basic_scheduler() -> eyre::Result<()> {
        #[derive(Clone, Debug)]
        struct Identity {}

        #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
        enum TaskLabel {
            Input,
            Identity,
            Combine,
        }

        impl std::fmt::Display for Identity {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", &self)
            }
        }

        #[async_trait::async_trait]
        impl Task1<String, String> for Identity {
            async fn run(self: Box<Self>, input: String) -> TaskResult<String> {
                println!("identity with input: {input:?}");
                Ok(input)
            }
        }

        #[derive(Clone, Debug)]
        struct Combine {}

        impl std::fmt::Display for Combine {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", &self)
            }
        }

        #[async_trait::async_trait]
        impl Task2<String, String, String> for Combine {
            async fn run(self: Box<Self>, a: String, b: String) -> TaskResult<String> {
                println!("combine with input: {:?}", (&a, &b));
                Ok(format!("{} {}", &a, &b))
            }
        }

        #[derive(Clone, Debug, Default)]
        struct CustomPolicy {}

        impl Policy<TaskLabel> for CustomPolicy {
            fn arbitrate(
                &self,
                exec: &dyn executor::Executor<TaskLabel>,
            ) -> Option<TaskRef<TaskLabel>> {
                let running_durations: Vec<_> = exec
                    .running()
                    .iter()
                    .filter_map(|t| t.started_at())
                    .map(|start_time| start_time.elapsed())
                    .collect();
                dbg!(running_durations);
                dbg!(exec.running().len());
                let num_combines = exec
                    .running()
                    .iter()
                    .filter(|t: &&TaskRef<TaskLabel>| *t.label() == TaskLabel::Combine)
                    .count();
                dbg!(num_combines);
                exec.ready().next()
            }
        }

        color_eyre::install()?;

        let combine = Combine {};
        let identity = Identity {};

        let mut graph = Schedule::default();

        let input_node =
            graph.add_node(TaskInput::from("George".to_string()), (), TaskLabel::Input);

        let base_node = graph.add_node(identity.clone(), (input_node,), TaskLabel::Identity);
        let parent1_node =
            graph.add_node(identity.clone(), (base_node.clone(),), TaskLabel::Identity);
        let parent2_node =
            graph.add_node(identity.clone(), (base_node.clone(),), TaskLabel::Identity);
        let result_node = graph.add_node(
            combine.clone(),
            (parent1_node, parent2_node),
            TaskLabel::Combine,
        );
        // dbg!(&graph);

        #[cfg(feature = "render")]
        graph.render_to(test_result_file!("graph.svg"))?;

        let mut executor = PolicyExecutor::custom(graph, CustomPolicy::default());

        // run all tasks
        executor.run().await;

        #[cfg(feature = "render")]
        executor
            .trace
            .render_to(test_result_file!("trace.svg"))
            .await?;

        // debug the graph now
        // dbg!(&executor.schedule);

        // assert the output value of the scheduler is correct
        assert_eq!(result_node.output(), Some("George George".to_string()));

        Ok(())
    }
}
