/// Compile-fail tests for `taski` invariants.
///
/// These are regular rustdoc tests using `compile_fail`.
///
/// # Cross-schedule mixing is rejected (different guards)
///
/// The generativity lifetime prevents mixing handles from different schedules.
///
/// ```compile_fail
/// use taski::{Schedule, TaskResult};
///
/// taski::make_guard!(guard_a);
/// taski::make_guard!(guard_b);
/// let mut a: Schedule<'_, ()> = Schedule::new(guard_a);
/// let mut b: Schedule<'_, ()> = Schedule::new(guard_b);
///
/// let a1 = a.add_input(1_u32, ());
///
/// async fn add_one(input: u32) -> TaskResult<u32> { Ok(input + 1) }
///
/// // This must not compile because `a1` belongs to a different schedule lifetime.
/// let _ = b.add_closure(add_one, (a1,), ());
/// ```
///
/// # Can’t construct `TaskId` from user code
///
/// `TaskId::new` is crate-private so you can’t forge task IDs.
///
/// ```compile_fail
/// use taski::dag;
///
/// let idx = dag::Idx::new(0);
/// let _task_id = dag::TaskId::new(1, idx);
/// ```
///
/// # Type-safe outputs
///
/// Output access via a typed `Handle<'id, O>` prevents mixing output types.
///
/// ```compile_fail
/// use taski::{PolicyExecutor, Schedule};
///
/// taski::make_guard!(guard);
/// let mut schedule = Schedule::new(guard);
///
/// let handle = schedule.add_input(1_u32, ());
///
/// let mut executor = PolicyExecutor::fifo(schedule);
/// let _: Option<&String> = executor.execution().output_ref(handle);
/// ```
///
/// # Schedule cannot be mutated through an executor
///
/// `PolicyExecutor::schedule()` returns `&Schedule`, so building additional nodes through the
/// executor is rejected by the borrow checker.
///
/// ```compile_fail
/// use taski::{PolicyExecutor, Schedule};
///
/// taski::make_guard!(guard);
/// let schedule: Schedule<'_, ()> = Schedule::new(guard);
/// let mut executor = PolicyExecutor::fifo(schedule);
/// executor.schedule().add_input(1_u32, ());
/// ```
///
/// # Priority scheduling requires an `Ord` label
///
/// ```compile_fail
/// use taski::{PolicyExecutor, Schedule};
///
/// #[derive(Debug)]
/// struct Label;
///
/// taski::make_guard!(guard);
/// let schedule: Schedule<'_, Label> = Schedule::new(guard);
/// let _ = PolicyExecutor::priority(schedule);
/// ```
///
/// # Dependency outputs must be `Clone`
///
/// The built-in `Dependencies` tuple impls clone upstream outputs to pass them as inputs.
/// Therefore, using a non-`Clone` output as a dependency is rejected.
///
/// ```compile_fail
/// use taski::{Schedule, TaskResult};
///
/// #[derive(Debug)]
/// struct NotClone;
///
/// taski::make_guard!(guard);
/// let mut schedule: Schedule<'_, ()> = Schedule::new(guard);
///
/// let h = schedule.add_input(NotClone, ());
///
/// async fn consume(_v: NotClone) -> TaskResult<()> { Ok(()) }
///
/// let _ = schedule.add_closure(consume, (h,), ());
/// ```
pub struct CompileFailTests;
