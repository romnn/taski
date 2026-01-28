//! Tasks and async closures.
//!
//! `taski` supports two ways of defining work nodes:
//!
//! - Implement one of the `TaskN` traits (recommended for reusable tasks).
//! - Pass an async closure to [`crate::Schedule::add_closure`] (convenient for small pipelines).
//!
//! Both styles ultimately produce a task that returns a [`Result`]. Returning an error will fail
//! the task and (by default) fail dependants.

use crate::{
    dag,
    dependency::Dependencies,
    execution::Execution,
    schedule::Schedulable,
    task,
};

use futures::Future;
use parking_lot::Mutex;
use std::any::Any;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

fn summarize(s: &dyn std::fmt::Debug, max_length: usize) -> String {
    let s = format!("{s:?}");
    let char_count = s.chars().count();
    if char_count <= max_length {
        return s;
    }

    let head_len = max_length / 2;
    let tail_len = max_length.saturating_sub(head_len);

    let head: String = s.chars().take(head_len).collect();
    let tail: String = s
        .chars()
        .skip(char_count.saturating_sub(tail_len))
        .collect();
    format!("{head}...{tail}")
}

/// A type-erased reference to a schedulable task node.
///
/// This is primarily used internally to store heterogeneous tasks in a single schedule DAG.
pub type Ref<'id, L> = Arc<dyn Schedulable<'id, L> + 'id>;

/// Task error wrapper.
///
/// `taski` stores task failures as boxed trait objects so tasks can return arbitrary error types.
/// Most users interact with errors via [`Result`].
pub struct Error(pub Box<dyn std::error::Error + Send + Sync + 'static>);

impl Error {
    /// Wraps an error value.
    pub fn new<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Error(Box::new(error))
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }

    #[allow(deprecated)]
    fn description(&self) -> &str {
        self.0.description()
    }

    #[allow(deprecated)]
    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.0.cause()
    }
}

impl From<Box<dyn std::error::Error + Send + Sync + 'static>> for Error {
    fn from(error: Box<dyn std::error::Error + Send + Sync + 'static>) -> Self {
        Self(error)
    }
}

/// Task result type used by `taski`.
///
/// Returning `Err(_)` fails the task.
pub type Result<O> = std::result::Result<O, Box<dyn std::error::Error + Send + Sync + 'static>>;

/// Boxed future returned by closure-based tasks.
pub type Fut<O> = Pin<Box<dyn Future<Output = Result<O>> + Send>>;

/// Internal abstraction for async closures used with [`crate::Schedule::add_closure`].
///
/// Users typically do not implement this trait manually. Instead, `taski` provides blanket
/// implementations via the `ClosureN` traits (`Closure1`..`Closure8`), allowing async closures of
/// arity 1..8 to be used as schedule nodes.
pub trait Closure<I, O> {
    /// Runs the closure, consuming it.
    fn run(self: Box<Self>, inputs: I) -> Fut<O>;
}

/// Pending task.
///
/// Due to lack of trait specialization, `Task` and `Closure` may overlap.
/// Hence, we resort to explicit static dispatch here.
pub(crate) enum PendingTask<I, O> {
    Task(Box<dyn Task<I, O> + Send + Sync>),
    Closure(Box<dyn Closure<I, O> + Send + Sync>),
}

impl<I, O> PendingTask<I, O> {
    /// Runs the pending task, consuming it.
    async fn run(self, input: I) -> Result<O> {
        match self {
            Self::Task(task) => task.run(input).await,
            Self::Closure(closure) => {
                let fut = closure.run(input);
                fut.await
            }
        }
    }
}

/// Internal state of a task
#[allow(dead_code)]
pub(crate) enum InternalState<I, O> {
    /// Task is pending and waiting to be run
    Pending(PendingTask<I, O>),
    /// Task is running
    Running,
    /// Task succeeded with the desired output
    Succeeded(O),
    /// Task failed with an error
    #[allow(unused)]
    Failed(Error),
}

#[allow(dead_code)]
impl<I, O> InternalState<I, O> {
    /// Whether the task is in `Pending` state.
    #[allow(unused)]
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    /// Whether the task is in `Running` state.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Whether the task is in `Succeeded` state.
    pub fn did_succeed(&self) -> bool {
        matches!(self, Self::Succeeded(_))
    }

    /// Whether the task is in `Failed` state.
    #[allow(unused)]
    pub fn did_fail(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// The state of a task.
#[derive(Debug, Clone)]
pub enum State {
    /// Task is pending.
    Pending,
    /// Task is running.
    Running,
    /// Task succeeded.
    Succeeded,
    /// Task failed with an error.
    Failed,
}

impl State {
    #[must_use]
    /// Whether the task is in `Pending` state.
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Whether the task is in `Running` state.
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    #[must_use]
    /// Whether the task is in `Succeeded` state.
    pub fn did_succeed(&self) -> bool {
        matches!(self, Self::Succeeded)
    }

    /// Whether the task is in `Failed` state.
    #[must_use]
    pub fn did_fail(&self) -> bool {
        matches!(self, Self::Failed)
    }
}

#[async_trait::async_trait]
/// Base task trait used by the executor.
///
/// Most users implement one of the `TaskN` traits instead of implementing this directly.
pub trait Task<I, O> {
    /// Runs the task.
    ///
    /// Running a task consumes it, which does guarantee that
    /// tasks may only run exactly once.
    async fn run(self: Box<Self>, input: I) -> Result<O>;

    /// The name of the task.
    ///
    /// This name is used for rendering.
    fn name(&self) -> String {
        "<unnamed>".to_string()
    }

    /// The color of the task when rendered.
    #[cfg_attr(docsrs, doc(cfg(feature = "render")))]
    fn color(&self) -> Option<crate::render::Rgba> {
        None
    }
}

/// A simple terminal input for a task
#[derive(Clone, Debug)]
pub struct Input<O>(O);

impl<O> From<O> for Input<O> {
    #[inline]
    fn from(value: O) -> Self {
        Self(value)
    }
}

/// Implements the Task trait for task inputs.
///
/// All that this implementation does is return a shared
/// reference to the task input.
#[async_trait::async_trait]
impl<O> Task0<O> for Input<O>
where
    O: std::fmt::Debug + Send + 'static,
{
    async fn run(self: Box<Self>) -> Result<O> {
        Ok(self.0)
    }

    fn name(&self) -> String {
        format!("{self}")
    }
}

impl<O> std::fmt::Display for Input<O>
where
    O: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = summarize(&self.0, 20);
        write!(f, "Input({value})")
    }
}

/// A task node in the task graph.
///
/// The task node tracks the state of the tasks lifecycle and is
/// assigned a unique index.
/// It is not possible to directly construct a task node to
/// enforce correctness.
/// References to `TaskNode` can be used as dependencies.
///
/// `TaskNodes` are only generic (static) over the inputs and
/// outputs, since that is of importance for using a task node
/// as a dependency for another task
pub(crate) struct NodeInner<I, O> {
    pub(crate) task: Option<PendingTask<I, O>>,
}

impl<I, O> NodeInner<I, O> {
    pub fn new<T>(task: T) -> Self
    where
        T: Task<I, O> + Send + Sync + 'static,
    {
        let task = PendingTask::Task(Box::new(task));
        Self {
            task: Some(task),
        }
    }

    pub fn closure<C>(closure: Box<C>) -> Self
    where
        C: Closure<I, O> + Send + Sync + 'static,
        I: Send + 'static,
    {
        let task = PendingTask::Closure(closure);
        Self {
            task: Some(task),
        }
    }
}

/// A task node stored in a schedule DAG.
///
/// This type is primarily used internally by [`crate::Schedule`] to store tasks (both
/// task-trait-based and closure-based) in a heterogeneous graph.
///
/// Most users should not need to construct `Node` values directly; use
/// [`crate::Schedule::add_node`] and [`crate::Schedule::add_closure`] instead.
pub struct Node<'id, I, O, L> {
    /// Display name of the task.
    ///
    /// For task-based nodes this is derived from [`Task::name`]. For closure-based nodes this
    /// defaults to `"<unnamed>"`.
    pub task_name: String,

    /// Optional color used for rendering.
    ///
    /// When the `render` feature is enabled, a default color may be derived from the task id.
    pub color: Option<crate::render::Rgba>,

    /// User-provided label used by scheduling policies.
    pub label: L,

    /// When this node was created.
    pub created_at: Instant,

    /// The ids of dependency tasks.
    pub dependency_task_ids: Vec<dag::TaskId<'id>>,

    /// Typed dependency extractor used to build input values at runtime.
    pub dependencies: Box<dyn Dependencies<'id, I> + Send + Sync + 'id>,

    /// The branded id of this node.
    pub index: dag::TaskId<'id>,

    pub(crate) inner: Arc<Mutex<NodeInner<I, O>>>,
}

impl<'id, I, O, L> Node<'id, I, O, L> {
    /// Creates a task-based node.
    pub fn new<T, D>(
        task: T,
        deps: D,
        dependency_task_ids: Vec<dag::TaskId<'id>>,
        label: L,
        index: dag::TaskId<'id>,
    ) -> Self
    where
        T: task::Task<I, O> + Send + Sync + 'static,
        D: Dependencies<'id, I> + Send + Sync + 'id,
    {
        let color = task.color();
        #[cfg(feature = "render")]
        let color = Some(color.unwrap_or_else(|| crate::render::color_from_id(index)));
        Self {
            task_name: task.name(),
            color,
            label,
            created_at: Instant::now(),
            dependency_task_ids,
            inner: Arc::new(Mutex::new(task::NodeInner::new(task))),
            dependencies: Box::new(deps),
            index,
        }
    }

    /// Creates a closure-based node.
    pub fn closure<C, D>(
        closure: Box<C>,
        deps: D,
        dependency_task_ids: Vec<dag::TaskId<'id>>,
        label: L,
        index: dag::TaskId<'id>,
    ) -> Self
    where
        C: Closure<I, O> + Send + Sync + 'static,
        D: Dependencies<'id, I> + Send + Sync + 'id,
        I: Send + 'static,
    {
        #[cfg(not(feature = "render"))]
        let color = None;
        #[cfg(feature = "render")]
        let color = Some(crate::render::color_from_id(index));
        Self {
            task_name: "<unnamed>".to_string(),
            color,
            label,
            created_at: Instant::now(),
            dependency_task_ids,
            inner: Arc::new(Mutex::new(task::NodeInner::closure(closure))),
            dependencies: Box::new(deps),
            index,
        }
    }
}

impl<I, O, L> Hash for Node<'_, I, O, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // hash the index
        self.index.hash(state);
    }
}

impl<I, O, L> std::fmt::Display for Node<'_, I, O, L>
where
    I: std::fmt::Debug + Send + Sync + 'static,
    O: std::fmt::Debug + Send + Sync + 'static,
    L: std::fmt::Debug + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
    }
}

#[async_trait::async_trait]
impl<'id, I, O, L> Schedulable<'id, L> for Node<'id, I, O, L>
where
    I: std::fmt::Debug + Send + Sync + 'static,
    O: std::fmt::Debug + Send + Sync + 'static,
    L: std::fmt::Debug + Send + Sync + 'static,
{
    fn succeeded(&self, execution: &Execution<'id>) -> bool {
        execution.state(self.index()).did_succeed()
    }

    fn fail(&self, execution: &mut Execution<'id>, err: Error) {
        execution.mark_failed(self.index(), Instant::now(), err);
    }

    fn state(&self, execution: &Execution<'id>) -> State {
        execution.state(self.index())
    }

    fn as_argument(&self, execution: &Execution<'id>) -> String {
        match execution.state(self.index()) {
            State::Pending => "<pending>".to_string(),
            State::Running => "<running>".to_string(),
            State::Succeeded => "<succeeded>".to_string(),
            State::Failed => "<failed>".to_string(),
        }
    }

    fn created_at(&self) -> Instant {
        self.created_at
    }

    fn started_at(&self, execution: &Execution<'id>) -> Option<Instant> {
        execution.started_at(self.index())
    }

    fn completed_at(&self, execution: &Execution<'id>) -> Option<Instant> {
        execution.completed_at(self.index())
    }

    fn index(&self) -> dag::TaskId<'id> {
        self.index
    }

    fn run(&self, execution: &Execution<'id>) -> Option<crate::schedule::Fut<'id>> {
        // get the inputs from the dependencies
        let inputs = self.dependencies.inputs(execution)?;

        let task = self.inner.lock().task.take()?;

        let idx = self.index();
        #[cfg(feature = "render")]
        let color = *self.color();
        let label = self.to_string();
        let start = execution.started_at(idx).unwrap_or_else(Instant::now);

        Some(Box::pin(async move {
            log::debug!("running {label}");

            // this will consume the task
            let result = task.run(inputs).await;

            let end = Instant::now();

            let result = result
                .map(|output| Arc::new(output) as Arc<dyn Any + Send + Sync>)
                .map_err(Error::from);

            let traced = crate::trace::Task {
                label,
                #[cfg(feature = "render")]
                color,
                start,
                end,
            };
            (idx, traced, result)
        }))
    }

    fn label(&self) -> &L {
        &self.label
    }

    fn name(&self) -> &str {
        &self.task_name
    }

    fn color(&self) -> &Option<crate::render::Rgba> {
        &self.color
    }

    fn dependencies(&self) -> &[dag::TaskId<'id>] {
        self.dependency_task_ids.as_slice()
    }
}

// TODO: add documentation
macro_rules! task {
    ($name:ident: $( $type:ident ),*) => {
        #[doc = concat!("A task with input(s) `", stringify!($( $type ),*), "` and output `O`.

**Note**: Using the [`async_trait`]: <https://docs.rs/async-trait/latest/async_trait/> macro, ", stringify!($name), " can be written more easily.





        ")]
        #[allow(
            non_snake_case,
            clippy::too_many_arguments,
            clippy::module_name_repetitions,
        )]
        #[async_trait::async_trait]
        pub trait $name<$( $type ),*, O>: std::fmt::Debug {

            #[doc = concat!("Runs the task with inputs: `", stringify!($( $type ),*), "` and output `O`.

The function **must** produce a `taski::task::Result<O>` to support fail-fast of dependent tasks.
However, if you wish to propagate errors to dependent tasks, you can always wrap your own
result, e.g. `taski::task::Result<Result<T, E>>`.
            ")]
            async fn run(self: Box<Self>, $($type: $type),*) -> Result<O>;

            fn name(&self) -> String {
                format!("{self:?}")
            }

            fn color(&self) -> Option<crate::render::Rgba> {
                None
            }

        }

        #[allow(non_snake_case, clippy::too_many_arguments)]
        #[async_trait::async_trait]
        impl<T, $( $type ),*, O> Task<($( $type ),*,), O> for T
        where
            T: $name<$( $type ),*, O> + Send + 'static,
            $($type: std::fmt::Debug + Send + 'static),*
        {
            async fn run(self: Box<Self>, input: ($( $type ),*,)) -> Result<O> {
                // destructure to tuple and call
                let ($( $type ),*,) = input;
                $name::run(self, $( $type ),*).await
            }

            fn name(&self) -> String {
                $name::name(self)
            }

            fn color(&self) -> Option<crate::render::Rgba> {
                $name::color(self)
            }
        }
    }
}

task!(Task1: T1);
task!(Task2: T1, T2);
task!(Task3: T1, T2, T3);
task!(Task4: T1, T2, T3, T4);
task!(Task5: T1, T2, T3, T4, T5);
task!(Task6: T1, T2, T3, T4, T5, T6);
task!(Task7: T1, T2, T3, T4, T5, T6, T7);
task!(Task8: T1, T2, T3, T4, T5, T6, T7, T8);

#[async_trait::async_trait]
#[allow(clippy::module_name_repetitions)]
/// Task with zero inputs.
///
/// This is useful for source nodes and constant producers.
pub trait Task0<O>: std::fmt::Debug {
    /// Runs the task.
    async fn run(self: Box<Self>) -> Result<O>;

    fn name(&self) -> String {
        format!("{self:?}")
    }

    fn color(&self) -> Option<crate::render::Rgba> {
        None
    }
}

#[async_trait::async_trait]
impl<T, O> Task<(), O> for T
where
    T: Task0<O> + Send + 'static,
{
    async fn run(self: Box<Self>, _: ()) -> Result<O> {
        Task0::run(self).await
    }

    fn name(&self) -> String {
        Task0::name(self)
    }

    fn color(&self) -> Option<crate::render::Rgba> {
        Task0::color(self)
    }
}

// TODO: add documentation
macro_rules! closure {
    ($name:ident: $( $type:ident ),*) => {
        #[doc = concat!(
"An async closure with input(s) `", stringify!($( $type ),*), "` and output `O`.

This trait is implemented automatically for `async` closures of the corresponding arity, allowing
them to be passed to [`crate::Schedule::add_closure`].
")]
        #[allow(
            non_snake_case,
            clippy::too_many_arguments,
            clippy::module_name_repetitions,
        )]
        #[async_trait::async_trait]
        pub trait $name<$( $type ),*, O> {
            #[doc = concat!(
"Runs the closure with inputs: `", stringify!($( $type ),*), "` and output `O`.

The returned future must resolve to [`crate::task::Result<O>`].
")]
            fn run(self: Box<Self>, $($type: $type),*) -> Fut<O>;
        }

        #[allow(non_snake_case, clippy::too_many_arguments)]
        #[async_trait::async_trait]
        impl<F, C, $( $type ),*, O> $name<$( $type ),*, O> for C
        where
            C: FnOnce($( $type ),*) -> F,
            F: Future<Output = Result<O>> + Send + 'static,
        {
            fn run(self: Box<Self>, $($type: $type),*) -> Fut<O> {
                Box::pin(self($( $type ),*))
            }
        }

        #[allow(non_snake_case, clippy::too_many_arguments)]
        #[async_trait::async_trait]
        impl<C, $( $type ),*, O> Closure<($( $type ),*,), O> for C
        where
            C: $name<$( $type ),*, O>,
        {
            fn run(self: Box<Self>, input: ($( $type ),*,)) -> Fut<O> {
                // destructure to tuple and call
                let ($( $type ),*,) = input;
                let fut = C::run(self, $( $type ),*);
                fut
            }
        }
    }
}

closure!(Closure1: T1);
closure!(Closure2: T1, T2);
closure!(Closure3: T1, T2, T3);
closure!(Closure4: T1, T2, T3, T4);
closure!(Closure5: T1, T2, T3, T4, T5);
closure!(Closure6: T1, T2, T3, T4, T5, T6);
closure!(Closure7: T1, T2, T3, T4, T5, T6, T7);
closure!(Closure8: T1, T2, T3, T4, T5, T6, T7, T8);

/// Supports `|| async {}` closures without any arguments.
#[async_trait::async_trait]
impl<F, C, O> Closure<(), O> for C
where
    C: FnOnce() -> F,
    F: Future<Output = Result<O>> + Send + 'static,
{
    fn run(self: Box<Self>, (): ()) -> Fut<O> {
        Box::pin(self())
    }
}
