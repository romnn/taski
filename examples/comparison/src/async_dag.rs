#![allow(clippy::just_underscores_and_digits, clippy::used_underscore_binding)]

use async_dag::Graph;

// Example taken from: https://github.com/chubei-oppen/async_dag

async fn sum(lhs: i32, rhs: i32) -> i32 {
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    lhs + rhs
}

pub async fn run() -> Option<i32> {
    let mut graph = Graph::new();
    // The closures are not run yet.
    let _1 = graph.add_task(|| async { 1 });
    let _2 = graph.add_task(|| async { 2 });
    let _4 = graph.add_task(|| async { 4 });

    // sets _1 as _3's first parameter
    let _3 = graph.add_child_task(_1, sum, 0).unwrap();

    // sets _2 as _3's second parameter
    graph.update_dependency(_2, _3, 1).unwrap();

    // sets _3 as _7's first parameter
    let _7 = graph.add_child_task(_3, sum, 0).unwrap();

    // sets _4 as _7's second parameter
    graph.update_dependency(_4, _7, 1).unwrap();

    // Runs all the tasks with maximum possible parallelism.
    graph.run().await;

    graph.get_value::<i32>(_7)
}

#[cfg(test)]
mod tests {
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn it_works() {
        assert_eq!(super::run().await, Some(7));
    }
}
