#![allow(clippy::missing_panics_doc)]

pub mod dag;
pub mod dependency;
pub mod executor;
pub mod policy;
pub mod schedule;
pub mod task;
pub mod trace;

pub use crate::dependency::Dependency;
pub use executor::PolicyExecutor;
pub use policy::Policy;
pub use schedule::Schedule;
pub use task::{Error, Input as TaskInput, Ref as TaskRef, Result as TaskResult, Task1, Task2};

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
            let manifest_dir = std::path::PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));
            let function_name = function_name!();
            let function_name = function_name
                .strip_suffix("::{{closure}}")
                .unwrap_or(function_name);
            let test = format!("{}_{}", function_name, $suffix);
            manifest_dir.join("src/tests/").join(test)
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

        // impl std::fmt::Display for Identity {
        //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //         write!(f, "{:?}", &self)
        //     }
        // }

        #[async_trait::async_trait]
        impl Task1<String, String> for Identity {
            async fn run(self: Box<Self>, input: String) -> TaskResult<String> {
                println!("identity with input: {input:?}");
                Ok(input)
            }
        }

        #[derive(Clone, Debug)]
        struct Combine {}

        // impl std::fmt::Display for Combine {
        //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //         write!(f, "{:?}", &self)
        //     }
        // }

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
                ready: &[dag::Idx],
                schedule: &schedule::Schedule<TaskLabel>,
            ) -> Option<dag::Idx> {
                let running_durations: Vec<_> = schedule
                    .running()
                    .filter_map(|idx| schedule.dag[idx].started_at())
                    .map(|start_time| start_time.elapsed())
                    .collect();
                dbg!(running_durations);
                dbg!(schedule.running().count());
                let num_combines = schedule
                    .running()
                    .filter(|&idx| *schedule.dag[idx].label() == TaskLabel::Combine)
                    .count();
                dbg!(num_combines);
                ready.iter().next().copied()
            }
        }

        color_eyre::install()?;

        let combine = Combine {};
        let identity = Identity {};

        let mut graph = Schedule::default();

        let i0 = graph.add_input("George".to_string(), TaskLabel::Input);

        let n0 = graph.add_node(identity.clone(), (i0,), TaskLabel::Identity);
        let n1 = graph.add_node(identity.clone(), (n0.clone(),), TaskLabel::Identity);
        let n2 = graph.add_node(identity.clone(), (n0.clone(),), TaskLabel::Identity);
        let result_node = graph.add_node(combine.clone(), (n1, n2), TaskLabel::Combine);
        // dbg!(&graph);

        let mut executor = PolicyExecutor::custom(graph, CustomPolicy::default());

        // run all tasks
        executor.run().await;

        #[cfg(feature = "render")]
        if should_render() {
            let path = test_result_file!("graph.svg");
            println!("rendering graph to {}", path.display());
            executor.schedule.render_to(path)?;

            let path = test_result_file!("trace.svg");
            println!("rendering trace to {}", path.display());
            executor.trace.render_to(path)?;
        }

        // debug the graph now
        // dbg!(&executor.schedule);

        // assert the output value of the scheduler is correct
        let output = result_node.output();
        assert_eq!(output.as_deref(), Some("George George"));

        Ok(())
    }
}
