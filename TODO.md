#### TODO

I’ll do a pass over the [taski](cci:9://file:///home/roman/dev/luup/vendor/taski:0:0-0:0) sources to systematically surface sharp edges (panics, TODOs, invariants violated by public fields, etc.), then I’ll give you a prioritized review split into: critical correctness/safety bugs, API/UX issues, architectural improvements, and performance opportunities.





## 1) Bugs / safety / correctness issues (highest priority)

### **[Critical: failure path panics]** [Schedule::fail_dependants](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:276:4-366:5) is incomplete and currently crashes
- **What**: `schedule.rs: fail_dependants(..., dependencies: bool)` contains:
  - `dbg!` calls
  - `todo!()` when `dependencies == true`
- **Why it matters**: [PolicyExecutor::run()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/old/mod.rs:145:4-333:5) calls [self.schedule.fail_dependants(idx, true)](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:276:4-366:5) on any task failure. That means **any real failure will currently panic**.
- **Fix direction**:
  - Remove `dbg!` and implement the `dependencies == true` branch (or remove the parameter and implement one clear behavior).
  - Define a clear failure model: “fail downstream only” vs “also fail upstream when no longer needed” (the latter is unusual).

### **[Critical: crate likely does not compile as-is]** Empty derives: `#[derive()]`
- **What**: [task.rs](cci:7://file:///home/roman/dev/luup/vendor/taski/src/task.rs:0:0-0:0) contains `#[derive()]` on [NodeInner](cci:2://file:///home/roman/dev/luup/vendor/taski/src/task.rs:251:0-255:1) and [Node](cci:2://file:///home/roman/dev/luup/vendor/taski/src/task.rs:287:0-296:1).
- **Why it matters**: An empty derive attribute is a **hard compile error**.
- **Fix direction**: Remove those derives or replace with the intended derive list.

### **[Critical: invariants can be violated via public fields]** `Schedule.dag` is `pub`
- **What**: `Schedule<L> { pub dag: DAG<task::Ref<L>> }` exposes the underlying `petgraph` directly.
- **Why it matters**:
  - Users can `remove_node`, `add_edge`, etc. which breaks invariants that [add_node](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:198:4-225:5) assumes (e.g. `assert_eq!(node_index, index)` relies on no removals / stable indexing assumptions).
  - Users can create cycles or inconsistent edges, undermining “DAG by construction”.
- **Fix direction**:
  - Make `dag` private and expose a controlled API (query iterators, [node(id)](cci:1://file:///home/roman/dev/luup/vendor/taski/src/render.rs:276:12-291:13), `neighbors`, etc.).
  - If you want “power user escape hatch”, gate `dag_mut()` behind an `unsafe`-ish feature (`unstable` / `raw_graph`) and document invariants.

### **[Critical: cross-schedule dependency corruption]** Dependencies are not proven to belong to the same schedule
- **What**: You can pass a node from schedule A as a dependency while adding a node to schedule B.
- **Why it matters**:
  - If the `NodeIndex` exists in B, you silently wire to the wrong node.
  - If it doesn’t exist, `petgraph` will panic when adding an edge.
- **Fix direction** (strongly recommended if you want “best-in-class type safety”):
  - Introduce a **schedule identity/token** stored in each node and validated in [add_node](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:198:4-225:5).
  - Or redesign handles so a node handle is parameterized by a schedule lifetime/token (harder but very strong correctness).

### **[Critical: unsafe-by-API / Send issues]** Type erasure drops `Send`/`Sync`
- **What**:
  - `task::Ref<L> = Arc<dyn Schedulable<L>>` (no `Send + Sync`)
  - `schedule::Fut` is not `Send`
- **Why it matters**: This makes it much harder to use `taski` robustly in multi-threaded async contexts (Tokio multi-thread), and can prevent [Schedule](cci:2://file:///home/roman/dev/luup/vendor/taski/src/old/schedule.rs:28:0-33:1) / executor futures from being `Send`.
- **Fix direction**:
  - Make erased types `Arc<dyn Schedulable<L> + Send + Sync>`
  - Make scheduled task futures `Pin<Box<dyn Future<...> + Send + 'static>>`

### **[Correctness: panic on non-UTF8 boundary slicing]** [summarize()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/task.rs:13:0-24:1) can panic
- **What**: [task.rs::summarize](cci:1://file:///home/roman/dev/luup/vendor/taski/src/task.rs:13:0-24:1) slices strings by byte indices (`&s[..]`), which can split a UTF-8 codepoint.
- **Impact**: Debug output containing non-ASCII can crash rendering / formatting ([Input](cci:2://file:///home/roman/dev/luup/vendor/taski/src/task.rs:202:0-202:23) display uses this).
- **Fix direction**: Truncate via `char_indices()` boundaries (or avoid truncation-by-slice entirely).

### **[Correctness/API semantics]** “ready” does not mean “pending & runnable”
- **What**: [Schedulable::is_ready()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:105:4-110:5) only checks “all deps succeeded”; it does **not** check the node’s own state.
- **Impact**:
  - Already-succeeded nodes can appear as “ready” again (executor tolerates it because [run()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/old/mod.rs:145:4-333:5) returns `None`, but policies/consumers may assume ready == runnable).
  - More wasted scheduling work / confusing semantics.
- **Fix direction**: Define “ready” as `state == Pending && deps_succeeded`.

### **[Bug: typo]** [Trace::duraton](cci:1://file:///home/roman/dev/luup/vendor/taski/src/trace.rs:13:4-17:5)
- **What**: [trace.rs](cci:7://file:///home/roman/dev/luup/vendor/taski/src/trace.rs:0:0-0:0) method is spelled [duraton](cci:1://file:///home/roman/dev/luup/vendor/taski/src/trace.rs:13:4-17:5).
- **Fix direction**: Rename to `duration` (and keep a deprecated alias if semver matters).

### **[Render feature: many unwraps & potential overflow]**
- **What**: [render.rs](cci:7://file:///home/roman/dev/luup/vendor/taski/src/render.rs:0:0-0:0) uses multiple `.unwrap()` and `Duration::from_nanos(latest.try_into().unwrap())`.
- **Impact**: Rendering can panic on large traces or IO/plotter failures; this is especially painful because rendering is often used for debugging.
- **Fix direction**: Convert to fallible paths and return structured errors consistently.

### **[Lock poisoning / panic amplification]** pervasive `RwLock::read().unwrap()`
- **What**: [task.rs](cci:7://file:///home/roman/dev/luup/vendor/taski/src/task.rs:0:0-0:0) uses `std::sync::RwLock` and unwraps lock acquisition.
- **Impact**: Any panic while holding the lock poisons it → future reads panic → cascading failure.
- **Fix direction**:
  - Use `parking_lot::RwLock` (no poisoning) or handle `PoisonError` explicitly.
  - Consider avoiding locks in hot paths entirely (see performance section).

---

## 2) Improvements: architecture, modularity, DX, correctness, performance

### **[Architecture: separate “graph definition” from “execution state”]**
Right now each node owns mutable execution state (`Pending/Running/Succeeded/Failed`, timestamps) inside the graph. That makes:
- reuse of the DAG shape difficult (can’t re-run without rebuilding)
- alternative executors harder (state model is embedded in node)

**Suggested design**:
- [Schedule](cci:2://file:///home/roman/dev/luup/vendor/taski/src/old/schedule.rs:28:0-33:1) becomes an immutable *definition graph*: tasks + dependency wiring + labels.
- An `Execution` (or `RunContext`) owns per-node runtime state (status, timestamps, outputs).
- This also makes it easier to add features like cancellation, retries, incremental runs, and deterministic replay.

### **[Type safety: schedule-owned handles]**
If the goal is “best type-safe DAG crate”, this is the biggest win:
- Introduce a `NodeHandle<I, O, L, S>` where `S` ties it to a schedule identity.
- This prevents cross-schedule mixing at compile time (or at least makes it detectable at runtime with an ID check).

### **[Policy API: make it stateful + event-driven]**
Current trait: `fn arbitrate(&self, ready, schedule) -> Option<Idx>`
- **Limitations**:
  - Hard to implement policies that need internal state (fairness, aging, quotas) without interior mutability.
  - Policies repeatedly call [schedule.running().count()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/old/mod.rs:130:4-133:5) which is expensive and lock-heavy.
- **Suggested evolution**:
  - `fn arbitrate(&mut self, ready, ctx) -> Decision`
  - Provide hooks: `on_task_started`, `on_task_completed`
  - Pass precomputed summaries: running counts per label, ready metadata snapshot, etc.

### **[Executor API / DX: return an execution report]**
Currently [PolicyExecutor::run()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/old/mod.rs:145:4-333:5) returns `()`. Users must manually inspect nodes to find failures.
- Provide something like:
  - `ExecutionReport { status_by_node, trace, roots, failed, skipped, ... }`
- Include failure causes (failed dependency chain) and final outputs for requested roots.

### **[Core DX mismatch]** `Task0..Task8` exist, but [Dependencies](cci:2://file:///home/roman/dev/luup/vendor/taski/src/dependency.rs:8:0-11:1) only supports 0–2 tuple deps
- This is a big usability cliff for a “typed DAG” crate.
- Fix with a macro generating [Dependencies](cci:2://file:///home/roman/dev/luup/vendor/taski/src/dependency.rs:8:0-11:1) impls for tuples up to N (8 or 12).

### **[Remove/feature-gate “old/” and “deprecated/”]**
- Shipping large experimental code inside [src/old](cci:9://file:///home/roman/dev/luup/vendor/taski/src/old:0:0-0:0) and [src/deprecated](cci:7://file:///home/roman/dev/luup/vendor/taski/src/deprecated:0:0-0:0) increases:
  - compile time
  - cognitive load
  - maintenance burden
- Recommendation:
  - move them behind a feature (`unstable` / `experiments`) or remove from the published crate.

---

## 3) Performance and scalability concerns

### **[Hot-path allocations]** [dependencies()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/task.rs:489:4-491:5) allocates a new `Vec` on every call
- [Schedulable::dependencies() -> Vec<Arc<...>>](cci:1://file:///home/roman/dev/luup/vendor/taski/src/task.rs:489:4-491:5) forces allocations.
- [is_ready()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:105:4-110:5) calls [dependencies()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/task.rs:489:4-491:5); readiness checks become allocation-heavy.
- **Fix direction**:
  - store dependencies in the node as `Vec<TaskRef>` once (for scheduling)
  - keep typed dependency extraction separately (for inputs)
  - or change trait to expose an iterator/slice view.

### **[O(n) scheduling operations]**
- `ready.contains`, `ready.retain` are O(n).
- Policies calling [schedule.running().count()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/old/mod.rs:130:4-133:5) scan the whole graph and lock each node.
- **Fix direction**:
  - Use a proper queue structure (`VecDeque`) or indexed heap/priority queue.
  - Track running counts incrementally in the executor.
  - Maintain indegree/remaining-deps counters for faster readiness (classic DAG execution strategy).

### **[Lock choice]**
- `std::sync::RwLock` in async code can be okay for very short critical sections, but it’s still a blocking primitive.
- For a high-quality async execution crate, prefer:
  - lock-free state (atomics + once-cell-like output storage), or
  - `parking_lot` locks (fast, no poisoning), or
  - executor-owned state (no per-node locks needed).

---

## Recommended next-step order (pragmatic)

1. **Make failure execution correct**: implement failure propagation properly (remove `todo!`, remove `dbg!`, define semantics).
2. **Fix compile breakers & sharp panics**: `#[derive()]`, UTF-8 slicing, [Trace::duraton](cci:1://file:///home/roman/dev/luup/vendor/taski/src/trace.rs:13:4-17:5), remove/handle unwraps where appropriate.
3. **Seal invariants**: make `Schedule.dag` private; prevent cross-schedule dependencies.
4. **Make it truly multi-thread async-friendly**: ensure erased handles + futures preserve `Send + Sync`.
5. **Finish the typed surface**: generate [Dependencies](cci:2://file:///home/roman/dev/luup/vendor/taski/src/dependency.rs:8:0-11:1) tuple impls up to 8+.
6. **Architecture upgrade** (optional but “best in ecosystem”): separate schedule definition from execution state; evolve policy/executor traits.

## Status
- **Done:** thorough review and prioritized findings across correctness, safety, API/DX, architecture, and performance.
- **Next:** tell me which direction you want first (e.g. “fix correctness only”, or “start the big type-safety redesign”), and I’ll propose a concrete implementation plan + patches.

# OLD TODO's

- allow users to specify custom colors
- support dynamic arguments too
- add examples of competitor libs (contrast with `async_dag`)
- add embedme
- add documentation
- add one more example
- improve the layout library using plotters

- NO: add a builder for the scheduler
- NO: add a schedule trait
- DONE: add more tests for different policies: fifo, priority, custom
- DONE: use logging
- DONE: remove the trace mutex? should not be required
- DONE: add support for async closures / functions
- DONE: use petgraph for the graph representation
