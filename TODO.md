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

# V1.1 TODO

I’ll do a structured “v1 readiness” review: first I’ll map the crate’s public API and core runtime flow (Schedule → Executor → Execution), then I’ll scan for correctness/soundness hazards (panic paths, inconsistent state transitions, concurrency/locking), and finally I’ll propose a concrete v1 plan with prioritized milestones (API stability, tests, docs, performance, and failure semantics). I’m going to start by surveying the crate sources and existing TODO/README.







# Crate review (current state)

## Architecture & data flow (what’s there now)
- **[Schedule<'id, L>](cci:2://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:180:0-183:1)**  
  - Owns the DAG definition (`petgraph::StableDiGraph`) of `task::Ref<'id, L>` (type-erased schedulables).  
  - Enforces “DAG by construction” by only allowing edges from already-added nodes to newly-added nodes.
- **[Execution<'id>](cci:2://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:28:0-33:1)**  
  - Owns per-task runtime state: [state](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:117:4-123:5), timestamps, `output: Arc<dyn Any + Send + Sync>`, `error`, and counters ([running_count](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:107:4-110:5), [unfinished_count](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:112:4-115:5)).
  - Public typed output API is now:
    - [execution.output_ref(handle) -> Option<&O>](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:125:4-130:5)
    - [execution.output(handle) / take_output(handle) -> Option<O>](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:147:4-152:5) (move-out, succeeds when uniquely owned)
- **[task::Node<'id, I, O, L>](cci:2://file:///home/roman/dev/luup/vendor/taski/src/task.rs:287:0-297:1) / [Schedulable<'id, L>](cci:2://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:47:0-150:1)**
  - Node is schedule-side “callable”; it resolves inputs through [Dependencies](cci:2://file:///home/roman/dev/luup/vendor/taski/src/dependency.rs:2:0-5:1), consumes the task exactly once via `inner: RwLock<NodeInner<...>>`, and returns a future yielding `(TaskId, trace::Task, Result<Arc<dyn Any>, task::Error>)`.
- **[PolicyExecutor<'id, P, L>](cci:2://file:///home/roman/dev/luup/vendor/taski/src/executor.rs:7:0-12:1)**
  - Owns `schedule`, `execution`, `policy`, and `trace`.
  - Execution loop is efficient: remaining-deps counters + policy-managed ready queues, no O(n) ready list scans.
- **[Policy](cci:2://file:///home/roman/dev/luup/vendor/taski/src/policy.rs:4:0-23:1)**
  - Event-driven; good design for fairness/resource constraints without recomputation.
- **[Trace](cci:2://file:///home/roman/dev/luup/vendor/taski/src/trace.rs:22:0-25:1) / [render](cci:1://file:///home/roman/dev/luup/vendor/taski/src/render.rs:276:8-344:9)**
  - Trace is recorded during execution. Rendering is behind [render](cci:1://file:///home/roman/dev/luup/vendor/taski/src/render.rs:276:8-344:9) feature.

Overall: this is a strong baseline for a v1 DAG executor. The b5a/b5b/b5c direction (definition vs execution split + event policies) is the right architecture for long-term evolution.

---

# Findings: issues & improvement opportunities

## Correctness / soundness / invariants (highest impact)
- **[Executor/schedule mutability invariant is currently not enforced]**  
  - [PolicyExecutor](cci:2://file:///home/roman/dev/luup/vendor/taski/src/executor.rs:7:0-12:1) exposes `pub schedule: Schedule<...>`.  
  - A user can call [executor.schedule.add_node(...)](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:194:4-226:5) after the executor is constructed. [Execution::new(schedule.dag.node_count())](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:36:4-44:5) is sized once, so any later-added tasks will create **invalid task IDs** relative to `Execution.tasks`.  
  - Today this degrades into logged `"invalid task id"` errors + silent incorrect behavior. For v1, this must be made impossible or safely supported.

- **[Cross-schedule handle mixing is still possible in some cases]**  
  - The `generativity` branding helps, but it does **not** prevent mixing handles from two schedules created with the *same* `guard` (thus same `'id`). That can cause:
    - wrong wiring if `NodeIndex` happens to exist in the other schedule
    - or panic inside `petgraph` when adding an edge to a missing index
  - For a “typed DAG” crate, this is a key v1-quality issue. You want a hard guarantee here.

- **[Deadlock / stalled execution behavior is under-specified]**  
  - In [PolicyExecutor::run](cci:1://file:///home/roman/dev/luup/vendor/taski/src/task.rs:99:4-108:5), if `unfinished_count > 0` but `running_tasks` is empty and [policy.next_task(...)](cci:1://file:///home/roman/dev/luup/vendor/taski/src/policy.rs:70:4-88:5) yields nothing, the executor logs:
    - `log::error!("no running tasks but schedule is unfinished");` and breaks.
  - This should become a **well-defined terminal outcome** for v1:
    - cycle detected
    - all remaining tasks blocked by failed dependencies
    - policy starvation / concurrency limits
  - And the user should get a structured report, not a log line.

- **[Failure propagation semantics not locked down]**  
  - [Schedule::fail_dependants](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:277:4-316:5) still has TODO notes (“is this correct… test this”).  
  - For v1, you want explicit semantics:
    - fail-fast downstream only?
    - mark as `Failed` vs `Skipped`?
    - do we keep running independent branches after a failure?

## Public API / DX / ergonomics
- **[README is outdated vs the new API]**
  - It still references the old `Dependency`/[node.output()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:147:4-152:5) pattern and the old policy API. This will confuse users immediately.
- **[Public surface likely too wide / exposes internals]**
  - Re-exporting `task::Ref` (`TaskRef`), [Schedulable](cci:2://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:47:0-150:1), etc. increases the “stability burden” for v1.
  - [PolicyExecutor](cci:2://file:///home/roman/dev/luup/vendor/taski/src/executor.rs:7:0-12:1) fields being public similarly creates accidental coupling and the mutability bug above.
- **[Execution output move-out has surprising semantics]**
  - [execution.output(handle)](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:147:4-152:5) can return `None` even when the task succeeded, if the output `Arc` is not uniquely owned (`Arc::try_unwrap` fails).
  - This is fine as an optimization, but it must be clear in v1 docs and probably should be surfaced as a distinct outcome (e.g. “not uniquely owned”).

## Performance / allocations
- **[Per-task lock on start]**  
  - [Node::run()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/task.rs:99:4-108:5) takes the task via `RwLock` even though execution is single-start per task. This is probably OK, but for v1 “polish” you could:
    - replace with `parking_lot::Mutex<Option<PendingTask<...>>>` (cheaper, simpler)
    - or redesign so the executor takes `&mut` access and no lock is needed (bigger change)
- **[Trace concurrency iteration clones a `HashSet` every step]**
  - [Trace::iter_concurrent()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/trace.rs:116:4-123:5) yields `Some(self.running.clone())` each event. Fine for tests/diagnostics, but could be expensive for large traces.
- **[Render graph [render()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/render.rs:276:8-344:9) returns `String` and can produce odd output for empty graphs]**
  - Not a correctness issue for the core runtime, but worth tightening before v1.

## Render feature robustness
- Rendering is “debug UX”; it should strive to be **panic-free and reliable** even on edge cases (empty schedule/trace, huge timestamps, etc.).  
  (Right now it looks mostly careful, but it would benefit from explicit “empty trace/graph” handling and consistent error returns in the render modules.)

---

# Concrete plan to get to a great v1

## Milestone 1 — **Stabilize and shrink the public API (v1 API boundary)**
- **[Make [PolicyExecutor](cci:2://file:///home/roman/dev/luup/vendor/taski/src/executor.rs:7:0-12:1) fields private]**
  - Provide `schedule()` / `schedule_mut()` only if safe, or make schedule immutable once in executor.
  - Provide `execution()` / `execution_mut()` carefully (mut access mainly for [output()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:147:4-152:5)/[take_output()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:144:4-166:5)).
- **[Hide internal traits/types from the public surface]**
  - Keep [Schedulable](cci:2://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:47:0-150:1) and [task::Node](cci:2://file:///home/roman/dev/luup/vendor/taski/src/task.rs:287:0-297:1) crate-private unless you *want* a plugin system where users implement custom node types (that’s a big v1 commitment).
- **[Publish a small “core public API”]**
  - [Schedule](cci:2://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:180:0-183:1), [Handle](cci:2://file:///home/roman/dev/luup/vendor/taski/src/dag.rs:13:0-16:1), [PolicyExecutor](cci:2://file:///home/roman/dev/luup/vendor/taski/src/executor.rs:7:0-12:1), [Policy](cci:2://file:///home/roman/dev/luup/vendor/taski/src/policy.rs:4:0-23:1), [Execution](cci:2://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:28:0-33:1) (read-only + output access), `TaskN`/`ClosureN`, `TaskResult`, [task::Error](cci:2://file:///home/roman/dev/luup/vendor/taski/src/task.rs:36:0-36:73).
- **[Update README + docs.rs]**
  - Ensure examples match the new output/policy APIs.
  - Add “quick start” that mirrors `examples/comparison`.

**Exit criteria**
- Public API feels intentional, minimal, hard to misuse.
- README/examples compile and match the crate.

---

## Milestone 2 — **Enforce invariants (no footguns)**
- **[Prevent schedule mutation after executor creation]** (pick one)
  - Option A: make executor constructors consume schedule and do not expose `&mut Schedule` afterward.
  - Option B: support dynamic schedule growth by making [Execution](cci:2://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:28:0-33:1) resize with schedule changes (complex).
  - For v1 I strongly recommend **Option A**.
- **[Prevent cross-schedule dependency mixing robustly]**
  - Add a **schedule instance ID** stored in [Schedule](cci:2://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:180:0-183:1) and embedded in [TaskId](cci:2://file:///home/roman/dev/luup/vendor/taski/src/dag.rs:7:0-10:1)/[Handle](cci:2://file:///home/roman/dev/luup/vendor/taski/src/dag.rs:13:0-16:1) (copyable), validated in:
    - [Schedule::add_node](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:194:4-226:5) / [add_closure](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:228:4-263:5)
    - possibly [Execution](cci:2://file:///home/roman/dev/luup/vendor/taski/src/execution.rs:28:0-33:1) accessors
  - This catches the “same guard reused” case.
- **[Define behavior for deadlock/stalled graphs]**
  - Detect “no progress possible” and return a structured error/report.
  - Optionally add a cycle detection pass at schedule build time (cheap for v1, very helpful).

**Exit criteria**
- Impossible (or loudly rejected) to construct invalid graphs via the safe API.
- Executor never “just logs and stops”; it returns a result.

---

## Milestone 3 — **Execution results + failure semantics (v1 UX)**
- **[Return a report from [run()](cci:1://file:///home/roman/dev/luup/vendor/taski/src/task.rs:99:4-108:5)]**
  - e.g. `ExecutionReport { finished: bool, failed: Vec<TaskId>, blocked: Vec<TaskId>, trace, ... }`
  - This is a major DX upgrade: users stop scraping internal state and logs.
- **[Finalize failure model]**
  - Decide and document:
    - fail-fast vs “continue independent branches”
    - represent blocked tasks as `Skipped` vs `FailedDependency` (likely add a state or at least a reason)
- **[Expose convenient query APIs]**
  - [Schedule::task_state(execution, task_id)](cci:1://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:356:4-359:5) exists; consider adding handle-based helpers.

**Exit criteria**
- Users can build, run, and understand outcomes without reading internal fields/logs.

---

## Milestone 4 — **Polish: performance + robustness + rendering**
- **[Reduce lock overhead in [Node::run](cci:1://file:///home/roman/dev/luup/vendor/taski/src/task.rs:99:4-108:5)]**
  - Replace `RwLock` with `Mutex<Option<...>>` (simple improvement).
- **[Make rendering robust on edge cases]**
  - Explicit handling of empty schedule/trace, avoid odd math/div-by-zero.
  - Ensure all render paths return errors rather than surprising output.
- **[Tighten trait bounds and docs]**
  - Align `TaskN` → [Task](cci:2://file:///home/roman/dev/luup/vendor/taski/src/task.rs:190:0-208:1) bridge bounds with what [Schedule](cci:2://file:///home/roman/dev/luup/vendor/taski/src/schedule.rs:180:0-183:1) requires (`Send + Sync`).

**Exit criteria**
- No obvious hotspots in typical workloads, rendering is reliable for debugging.

---

## Milestone 5 — **Test suite + release hardening**
- **[Add targeted tests for invariants]**
  - cross-schedule mixing rejection
  - schedule mutation after executor construction is impossible (compile-time) or returns error (runtime)
  - deadlock/cycle detection
  - failure propagation semantics
- **[Docs + examples as tests]**
  - ensure README examples are doctests (or copied into tests)
- **[Release checklist]**
  - `task check:fc` and `task test:fc` (feature combinations) green
  - docs build on docs.rs (with [render](cci:1://file:///home/roman/dev/luup/vendor/taski/src/render.rs:276:8-344:9) feature too)
  - semver review of public exports

**Exit criteria**
- Confidence that v1 behavior is stable and hard to misuse.

---

# Key v1 decision to make now
- **Dynamic graph building while executing** (mentioned as a goal in README):  
  I recommend **explicitly punting this to v2**, unless it’s a core requirement. It changes executor/schedule lifetimes and invariants significantly. For a “great v1”, a rock-solid “build then run” DAG executor is already very valuable.

---

# Current status
- The core architecture (Schedule/Execution split + event-driven policies + efficient executor loop) is in a strong place.
- The main remaining blockers for a “great v1” are:
  - **locking down invariants** (schedule mutability + cross-schedule handle safety)
  - **structured run outcome/reporting**
  - **documentation alignment**

If you tell me whether you want **compile-time prevention** vs **runtime validation** for cross-schedule mixing (I’d lean runtime schedule-id checks for practicality), I can refine Milestone 2 into an implementation-ready design with concrete API signatures.

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
