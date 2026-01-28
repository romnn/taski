#![allow(clippy::just_underscores_and_digits, clippy::used_underscore_binding)]

use taski::{PolicyExecutor, Schedule, TaskResult};

async fn sum_two_numbers(lhs: i32, rhs: i32) -> TaskResult<i32> {
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    Ok(lhs + rhs)
}

pub async fn run() -> Option<i32> {
    taski::make_guard!(guard);
    let mut graph = Schedule::new(guard);
    let _1 = graph.add_input(1, ());
    let _2 = graph.add_input(2, ());
    // note: `add_input` could also be written as a closure
    let _4 = graph.add_closure(|| async move { Ok(4) }, (), ());

    let closure = move |a, b| async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(a + b)
    };

    // sets _1 and _2 as _3's dependencies (arguments)
    // `add_closure` works for local async closures
    let _3 = graph.add_closure(closure, (_1, _2), ());

    // sets _3 and _4 as _7's dependencies (arguments)
    // `add_closure` also works with `async fn`
    let _7 = graph.add_closure(sum_two_numbers, (_3, _4), ());

    let mut executor = PolicyExecutor::fifo(graph);
    executor.run().await;

    // optional: render the DAG graph and an execution trace.
    super::render(&executor, "closures");

    executor.execution.output_ref(_7).cloned()
}

#[cfg(test)]
mod tests {
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn it_works() {
        assert_eq!(super::run().await, Some(7));
    }
}
