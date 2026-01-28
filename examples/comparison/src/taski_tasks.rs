#![allow(clippy::just_underscores_and_digits, clippy::used_underscore_binding)]

use color_eyre::eyre;
use taski::{PolicyExecutor, Schedule, TaskResult};

#[derive(Debug)]
struct SumTwoNumbers {}

/// Implement Task2 for `SumTwoNumbers`.
///
/// The first two generic arguments are the two arguments.
/// The last generic argument is the output type.
#[async_trait::async_trait]
impl taski::Task2<i32, i32, i32> for SumTwoNumbers {
    async fn run(self: Box<Self>, lhs: i32, rhs: i32) -> TaskResult<i32> {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(lhs + rhs)
    }
}

pub async fn run() -> eyre::Result<Option<i32>> {
    taski::make_guard!(guard);
    let mut graph = Schedule::new(guard);
    let _1 = graph.add_input(1, ());
    let _2 = graph.add_input(2, ());
    let _4 = graph.add_input(4, ());

    // sets _1 and _2 as _3's dependencies (arguments)
    let _3 = graph.add_node(SumTwoNumbers {}, (_1, _2), ())?;

    // sets _3 and _4 as _7's dependencies (arguments)
    let _7 = graph.add_node(SumTwoNumbers {}, (_3, _4), ())?;

    let mut executor = PolicyExecutor::fifo(graph);
    executor.run().await?;

    // optional: render the DAG graph and an execution trace.
    super::render(&executor, "tasks")?;

    Ok(executor.execution().output_ref(_7).copied())
}

#[cfg(test)]
mod tests {
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn it_works() {
        assert_eq!(super::run().await.unwrap(), Some(7));
    }
}
