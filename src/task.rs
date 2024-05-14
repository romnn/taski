use crate::{
    dag,
    dependency::{Dependencies, Dependency},
    schedule::Schedulable,
    task,
};

use futures::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::Instant;

fn summarize(s: &dyn std::fmt::Debug, max_length: usize) -> String {
    let s = format!("{s:?}");
    if s.len() > max_length {
        format!(
            "{}...{}",
            &s[..(max_length / 2)],
            &s[s.len() - (max_length / 2)..s.len()]
        )
    } else {
        s.to_string()
    }
}

pub type Ref<L> = Arc<dyn Schedulable<L>>;

pub type Result<O> = std::result::Result<O, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub type Fut<O> = Pin<Box<dyn Future<Output = Result<O>> + Send>>;

pub trait Closure<I, O> {
    fn run(self: Box<Self>, inputs: I) -> Fut<O>;
}

pub(crate) enum PendingTask<I, O> {
    Task(Box<dyn Task<I, O> + Send + Sync>),
    Closure(Box<dyn Closure<I, O> + Send + Sync>),
}

impl<I, O> PendingTask<I, O> {
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

// could change this to either closure or type here?
pub(crate) enum InternalState<I, O> {
    /// Task is pending and waiting to be run
    Pending(PendingTask<I, O>),
    /// Task is running
    Running,
    /// Task succeeded with the desired output
    Succeeded(O),
    /// Task failed with an error
    #[allow(unused)]
    Failed(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl<I, O> InternalState<I, O> {
    #[allow(unused)]
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    pub fn did_succeed(&self) -> bool {
        matches!(self, Self::Succeeded(_))
    }

    #[allow(unused)]
    pub fn did_fail(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

#[derive(Debug, Clone)]
pub enum State {
    /// Task is pending
    Pending,
    /// Task is running
    Running,
    /// Task succeeded
    Succeeded,
    /// Task failed with an error
    Failed,
}

impl State {
    #[must_use]
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    #[must_use]
    pub fn did_succeed(&self) -> bool {
        matches!(self, Self::Succeeded)
    }

    #[must_use]
    pub fn did_fail(&self) -> bool {
        matches!(self, Self::Failed)
    }
}

#[async_trait::async_trait]
pub trait Task<I, O> {
    /// Runs the task.
    ///
    /// Running a task consumes it, which does guarantee that
    /// tasks may only run exactly once.
    async fn run(self: Box<Self>, input: I) -> Result<O>;

    fn name(&self) -> String {
        "<unnamed>".to_string()
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
impl<O> Task<(), O> for Input<O>
where
    O: std::fmt::Debug + Send + 'static,
{
    async fn run(self: Box<Self>, _input: ()) -> Result<O> {
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

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("task dependency failed")]
    FailedDependency,
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
#[derive()]
pub(crate) struct NodeInner<I, O> {
    pub(crate) started_at: Option<Instant>,
    pub(crate) completed_at: Option<Instant>,
    pub(crate) state: InternalState<I, O>,
}

impl<I, O> NodeInner<I, O> {
    pub fn new<T>(task: T) -> Self
    where
        T: Task<I, O> + Send + Sync + 'static,
    {
        let task = PendingTask::Task(Box::new(task));
        let state = InternalState::Pending(task);
        Self {
            started_at: None,
            completed_at: None,
            state,
        }
    }

    pub fn closure<C>(closure: Box<C>) -> Self
    where
        C: Closure<I, O> + Send + Sync + 'static,
        I: Send + 'static,
    {
        let task = PendingTask::Closure(closure);
        let state = InternalState::Pending(task);
        Self {
            started_at: None,
            completed_at: None,
            state,
        }
    }
}

#[derive()]
pub struct Node<I, O, L> {
    pub task_name: String,
    pub label: L,
    pub created_at: Instant,
    pub dependencies: Box<dyn Dependencies<I, L> + Send + Sync>,
    pub index: dag::Idx,

    pub(crate) inner: RwLock<NodeInner<I, O>>,
}

impl<I, O, L> Node<I, O, L> {
    pub fn new<T, D>(task: T, deps: D, label: L, index: dag::Idx) -> Self
    where
        T: task::Task<I, O> + Send + Sync + 'static,
        D: Dependencies<I, L> + Send + Sync + 'static,
    {
        Self {
            task_name: task.name(),
            label,
            created_at: Instant::now(),
            inner: RwLock::new(task::NodeInner::new(task)),
            dependencies: Box::new(deps),
            index,
        }
    }

    pub fn closure<C, D>(closure: Box<C>, deps: D, label: L, index: dag::Idx) -> Self
    where
        C: Closure<I, O> + Send + Sync + 'static,
        D: Dependencies<I, L> + Send + Sync + 'static,
        I: Send + 'static,
    {
        Self {
            task_name: "<unnamed>".to_string(),
            label,
            created_at: Instant::now(),
            inner: RwLock::new(task::NodeInner::closure(closure)),
            dependencies: Box::new(deps),
            index,
        }
    }
}

impl<I, O, L> Hash for Node<I, O, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // hash the index
        self.index.hash(state);
    }
}

impl<I, O, L> std::fmt::Display for Node<I, O, L>
where
    I: std::fmt::Debug + Send + Sync + 'static,
    O: std::fmt::Debug + Send + Sync + 'static,
    L: std::fmt::Debug + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
    }
}

#[async_trait::async_trait]
impl<I, O, L> Schedulable<L> for Node<I, O, L>
where
    I: std::fmt::Debug + Send + Sync + 'static,
    O: std::fmt::Debug + Send + Sync + 'static,
    L: std::fmt::Debug + Sync + 'static,
{
    fn succeeded(&self) -> bool {
        let inner = self.inner.read().unwrap();
        inner.state.did_succeed()
    }

    fn fail(&self, err: Box<dyn std::error::Error + Send + Sync + 'static>) {
        let mut inner = self.inner.write().unwrap();
        inner.state = InternalState::Failed(err);
    }

    fn state(&self) -> State {
        let inner = self.inner.read().unwrap();
        match inner.state {
            InternalState::Pending(_) => State::Pending,
            InternalState::Running => State::Running,
            InternalState::Succeeded(_) => State::Succeeded,
            InternalState::Failed(_) => State::Failed,
        }
    }

    fn as_argument(&self) -> String {
        let inner = self.inner.read().unwrap();
        match inner.state {
            InternalState::Pending(_) => "<pending>".to_string(),
            InternalState::Running => "<running>".to_string(),
            InternalState::Succeeded(ref value) => format!("{value:?}"),
            InternalState::Failed(_) => "<failed>".to_string(),
        }
    }

    fn created_at(&self) -> Instant {
        self.created_at
    }

    fn started_at(&self) -> Option<Instant> {
        let inner = self.inner.read().unwrap();
        inner.started_at
    }

    fn completed_at(&self) -> Option<Instant> {
        let inner = self.inner.read().unwrap();
        inner.completed_at
    }

    fn index(&self) -> dag::Idx {
        self.index
    }

    async fn run(&self) {
        let task = {
            let mut inner = self.inner.write().unwrap();

            if let InternalState::Pending(_) = inner.state {
                // set the state to running
                // this takes ownership of the task
                let task = std::mem::replace(&mut inner.state, InternalState::Running);
                let InternalState::Pending(task) = task else {
                    return;
                };
                task
            } else {
                // already ran
                return;
            }
        };

        // get the inputs from the dependencies
        let inputs = self.dependencies.inputs().unwrap();

        {
            let mut inner = self.inner.write().unwrap();
            inner.started_at.get_or_insert(Instant::now());
        }

        println!("running task {self}");

        // this will consume the task
        let result = task.run(inputs).await;

        let mut inner = self.inner.write().unwrap();
        inner.completed_at.get_or_insert(Instant::now());

        assert!(inner.state.is_running());
        inner.state = match result {
            Ok(output) => InternalState::Succeeded(output),
            Err(err) => InternalState::Failed(err),
        };
    }

    fn label(&self) -> &L {
        &self.label
    }

    fn name(&self) -> &str {
        &self.task_name
    }

    fn dependencies(&self) -> Vec<Arc<dyn Schedulable<L>>> {
        self.dependencies.to_vec()
    }
}

impl<I, O, L> Dependency<O, L> for Node<I, O, L>
where
    I: std::fmt::Debug + Send + Sync + 'static,
    O: std::fmt::Debug + Clone + Send + Sync + 'static,
    L: std::fmt::Debug + Sync + 'static,
{
    fn output(&self) -> Option<O> {
        let inner = self.inner.read().unwrap();
        match inner.state {
            InternalState::Succeeded(ref output) => Some(output.clone()),
            _ => None,
        }
    }
}

// TODO: add documentation
macro_rules! task {
    ($name:ident: $( $type:ident ),*) => {
        #[allow(non_snake_case, clippy::too_many_arguments)]
        #[async_trait::async_trait]
        pub trait $name<$( $type ),*, O>: std::fmt::Debug {
            async fn run(self: Box<Self>, $($type: $type),*) -> Result<O>;

            fn name(&self) -> String {
                format!("{self:?}")
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
                format!("{self:?}")
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

// TODO: add documentation
macro_rules! closure {
    ($name:ident: $( $type:ident ),*) => {
        #[allow(non_snake_case, clippy::too_many_arguments)]
        #[async_trait::async_trait]
        pub trait $name<$( $type ),*, O> {
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
