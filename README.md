## taski

[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/romnn/taski/build.yaml?branch=main&label=build">](https://github.com/romnn/taski/actions/workflows/build.yaml)
[<img alt="test status" src="https://img.shields.io/github/actions/workflow/status/romnn/taski/test.yaml?branch=main&label=test">](https://github.com/romnn/taski/actions/workflows/test.yaml)
[![dependency status](https://deps.rs/repo/github/romnn/taski/status.svg)](https://deps.rs/repo/github/romnn/taski)
[<img alt="crates.io" src="https://img.shields.io/crates/v/taski">](https://crates.io/crates/taski)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/taski/latest?label=docs.rs">](https://docs.rs/taski)

`taski` is a small library for building and executing **async task DAGs**.

You describe your workflow as a directed acyclic graph (DAG) of tasks where:

- Each node is an async task (or an async closure).
- Edges describe typed dependencies (outputs of dependencies become inputs).
- An executor runs ready tasks concurrently.
- A policy decides *which* ready task to start next and *how much* concurrency you want.

Under the hood the graph is backed by **`petgraph`** (`StableDiGraph` + `NodeIndex`), which provides a solid, well-tested foundation.

```bash
cargo add taski
```

## Quickstart

This is the smallest “real” example: create a schedule, add some inputs, add a task that depends on them, run the executor, then read the output.

```rust
use taski::{PolicyExecutor, Schedule, TaskResult};

#[derive(Debug)]
struct Sum;

#[async_trait::async_trait]
impl taski::Task2<i32, i32, i32> for Sum {
    async fn run(self: Box<Self>, lhs: i32, rhs: i32) -> TaskResult<i32> {
        Ok(lhs + rhs)
    }
}

#[tokio::main]
async fn main() -> Result<(), taski::schedule::BuildError> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let one = schedule.add_input(1);
    let two = schedule.add_input(2);

    // `Sum` takes (i32, i32) and produces i32.
    let three = schedule.add_node(Sum, (one, two))?;

    let mut executor = PolicyExecutor::fifo(schedule);
    executor.run().await.ok();

    let output = executor.execution().output_ref(three).copied();
    assert_eq!(output, Some(3));

    Ok(())
}
```

You can also use `Schedule::add_closure(...)` to attach async closures or `async fn` directly (see `examples/comparison`).

## What you get

- **Typed dependencies.** Dependencies are expressed as tuples of previous nodes (e.g. `(a, b)`), and their outputs become the task input.
- **Async-first execution.** Tasks are `async` and executed using `futures::stream::FuturesUnordered`.
- **Pluggable scheduling policy.** Implement `taski::Policy<L>` to decide what to run next (FIFO, priority-by-metadata, custom constraints).
- **Configurable concurrency.** Built-in policies support a `max_concurrent` limit.
- **Tracing (and optional rendering).** The executor records a `Trace` (start/end times) and can be rendered to SVG with the `render` feature.
