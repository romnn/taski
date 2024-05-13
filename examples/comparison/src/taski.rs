#![allow(clippy::just_underscores_and_digits, clippy::used_underscore_binding)]

use std::path::PathBuf;
use taski::{Dependency, PolicyExecutor, Schedule};

fn manifest_dir() -> PathBuf {
    PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
}

// Optional: render the DAG graph and an execution trace.
//
// NOTE: this requires the "render" feature.
async fn render<P, L>(executor: &PolicyExecutor<P, L>) {
    executor
        .schedule
        .render_to(manifest_dir().join("taski_graph.svg"))
        .unwrap();
    executor
        .trace
        .render_to(manifest_dir().join("taski_trace.svg"))
        .await
        .unwrap();
}

#[derive(Debug)]
struct SumTwoNumbers {}

/// Implement Task2 for SumTwoNumbers.
///
/// The first two generic arguments are the two arguments.
/// The last generic argument is the output type.
#[async_trait::async_trait]
impl taski::Task2<i32, i32, i32> for SumTwoNumbers {
    async fn run(self: Box<Self>, lhs: i32, rhs: i32) -> taski::TaskResult<i32> {
        Ok(lhs + rhs)
    }
}

pub async fn run() -> Option<i32> {
    let mut graph = Schedule::default();
    let _1 = graph.add_input(1, ());
    let _2 = graph.add_input(2, ());
    let _4 = graph.add_input(4, ());

    // sets _1 and _2 as _3's dependencies (arguments)
    let _3 = graph.add_node(SumTwoNumbers {}, (_1, _2), ());

    // sets _3 and _4 as _7's dependencies (arguments)
    let _7 = graph.add_node(SumTwoNumbers {}, (_3, _4), ());

    let mut executor = PolicyExecutor::fifo(graph);
    executor.run().await;

    render(&executor).await;

    _7.output()
}

#[cfg(test)]
mod tests {
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn it_works() {
        assert_eq!(super::run().await, Some(7));
    }
}
