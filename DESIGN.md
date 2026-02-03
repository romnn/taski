## Core concepts

### `Schedule<L>`

`Schedule<L>` is the DAG of tasks.

If you don't need per-node metadata, use `Schedule<()>` and the `add_*` methods.

If you do need metadata (for custom scheduling policies or teardown coordination), use
`Schedule<L>` and the corresponding `*_with_metadata` methods.

- You add nodes via:
  - `Schedule::add_input(value)` for a constant value node.
  - `Schedule::add_node(task, deps)` for a typed task.
  - `Schedule::add_closure(closure, deps)` for an async closure or `async fn`.

  And (when using non-unit metadata):
  - `Schedule::add_input_with_metadata(value, metadata)`
  - `Schedule::add_node_with_metadata(task, deps, metadata)`
  - `Schedule::add_closure_with_metadata(closure, deps, metadata)`

The metadata (`L`) is user-defined data attached to each task node. It is intentionally part of the
core design because it enables powerful policies (priority, resource pools, per-metadata
concurrency, etc.) and higher-level orchestration (like teardown planning).

### Tasks: `Task0` … `Task8`

Tasks are written by implementing one of the `TaskN` traits (`Task0` has no inputs, `Task2` has two inputs, ...).

Internally, those traits bridge to a single trait (`taski::task::Task<I, O>`) by packing inputs into a tuple.

The schedule-side dependency plumbing is expressed through `taski::dependency::Dependencies<'id, I>`.

- `taski` ships tuple implementations for `()` and for tuples of handles up to 8 dependencies.

### Outputs and dependencies

Nodes returned by `add_input`/`add_node`/`add_closure` return a typed `Handle<'id, O>`.

- You can call `executor.execution().output_ref(handle)` after execution to borrow the output.
- Outputs are **cloned** when they are used as inputs for downstream tasks.
  - If outputs are large (or not cheaply cloneable), return `Arc<T>` (or another shared handle) from your tasks.

### `PolicyExecutor<P, L>`

The executor is intentionally separate from the schedule.

- `PolicyExecutor::fifo(schedule)` runs ready tasks in insertion order.
- `PolicyExecutor::priority(schedule)` runs the task with the highest metadata first (requires `L: Ord`).
- `PolicyExecutor::new(schedule, policy)` runs with any custom policy implementation.

### Policies

A policy is anything implementing:

```rust
pub trait Policy<'id, L> {
    fn reset(&mut self);

    fn on_task_ready(&mut self, task_id: taski::dag::TaskId<'id>, schedule: &taski::Schedule<'id, L>);

    fn on_task_started(&mut self, task_id: taski::dag::TaskId<'id>, schedule: &taski::Schedule<'id, L>);

    fn on_task_finished(
        &mut self,
        task_id: taski::dag::TaskId<'id>,
        state: taski::task::State,
        schedule: &taski::Schedule<'id, L>,
    );

    fn next_task(
        &mut self,
        schedule: &taski::Schedule<'id, L>,
        execution: &taski::execution::Execution<'id>,
    ) -> Option<taski::dag::TaskId<'id>>;
}
```

This API makes policies extremely flexible because they can inspect:

- The list of currently ready nodes.
- The full schedule and each node’s label/state.
- The set of currently running tasks (`schedule.running()`).

## How it works

### Graph representation (why `petgraph`)

`taski` stores tasks in a `petgraph::stable_graph::StableDiGraph`.

- Node IDs are `petgraph::graph::NodeIndex<usize>`.
- Each node stores an `Arc<dyn Schedulable<L>>` so the graph can hold heterogeneously-typed tasks.

### Internal task state (why you can’t construct nodes manually)

Each scheduled task node contains interior mutable state tracking:

- Pending / running / succeeded / failed.
- Start and completion timestamps.
- The original task (consumed when it runs).

This is also where the unique node index is assigned.

Because of that, the node type is **not something users are meant to construct directly**. Instead, you construct nodes through the schedule (`add_input`, `add_node`, `add_closure`) so that:

- The node is guaranteed to belong to the schedule.
- The node index is stable and unique.
- Dependencies always refer to nodes that are already in the schedule.

### “DAG-ness”

The public API enforces a DAG **by construction**: dependencies must already be present in the schedule, so edges always point from existing nodes to newly-added nodes.

This prevents forming cycles via safe public APIs.

If you build alternative constructors or mutate the underlying `petgraph` directly, you’ll need to reintroduce explicit cycle checks there.

### Execution loop

At a high level `PolicyExecutor::run()`:

- Collects all initially ready tasks (`schedule.ready()`).
- Repeatedly asks the policy to pick the next node from the ready queue.
- Starts the chosen task and tracks it in a `FuturesUnordered` pool.
- When a task completes:
  - Records timing info in `trace`.
  - If it succeeded, newly-unblocked dependants become ready.
  - If it failed, the executor follows a fail-fast strategy (dependent tasks can be failed due to failed dependencies).

`PolicyExecutor::run()` returns a `RunReport` on success and a `RunError` if execution stalls.

## Design decisions

This section preserves the original design notes, but with more context.

- **Task node is internal.** The node is the place where state is kept (interior mutability) and where the unique index is assigned.
  - Construction from the user is not the intended interface.
  - Nodes are created through `Schedule::{add_input, add_node, add_closure}`.
  - A node is the only valid dependency handle, because it proves:
    - The dependency was already added to the schedule.
    - Edges always go “forward in time”, so you can’t create cycles.

- **Tasks return `TaskResult<O>`.** Using a `Result` output is what enables fail-fast strategies and clean error propagation.
  - If you prefer not to fail fast, you can always make your task infallible and return a domain-specific value like `Option<O>` or `Result<T, E>` inside the task output.

- **Clone outputs when wiring inputs.** Downstream tasks get their inputs by cloning dependency outputs.
  - This is great for small values.
  - For large data (or non-cloneable data), return `Arc<O>` (or another shared handle) as the task output.

## Extensibility

`taski` is built around a small set of composable primitives.

- **Custom policies.** Implement `Policy<L>` to encode your own scheduling logic (priorities, resource classes, per-label concurrency, fairness, etc.).
- **Custom executors.** The executor is separate from `Schedule`, and `Schedule` exposes `ready()` and `running()` iterators plus node state. This makes it straightforward to build alternative executors (e.g. different failure handling, cancellation, instrumentation, integration with a custom runtime), while reusing the same graph and task definitions.

## Rendering and tracing (optional)

Enable the `render` feature to render:

- The schedule graph (`Schedule::render_to(...)`)
- The execution trace (`Trace::render_to(...)`)

The repository contains examples and committed SVGs under `src/tests/` and `examples/`.

## Development

`taski` uses macros to generate the `Task0..Task8` and `Closure1..Closure8` APIs. If you want to see the concrete code those macros expand to:

```bash
cargo install cargo-expand
cargo expand ::task
```
