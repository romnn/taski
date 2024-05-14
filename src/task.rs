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
    let s = format!("{:?}", s);
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

// pub type Fut<O> = Pin<Box<dyn Future<Output = Result<O>> + Send>>;
// pub type Fut<O> = Pin<Box<dyn Future<Output = Result<O>> + Unpin + Send>>;
pub type Fut<O> = Pin<Box<dyn Future<Output = Result<O>> + Send>>;

// #[async_trait::async_trait]

// pub trait Closure<F, I, O> {
pub trait Closure<I, O> {
    fn run(self: Box<Self>, inputs: I) -> Fut<O>;
    // fn run(self, inputs: I) -> Fut<O>;
}

// pub trait Closure<F, I, O>: std::ops::FnOnce(I) -> F + Unpin
// where
//     // I: 'static,
//     F: Future<Output = Result<O>>,
// {
// }
//
// impl<C, F, I, O> Closure<F, I, O> for C
// where
//     C: std::ops::FnOnce(I) -> F + Unpin,
//     F: Future<Output = Result<O>>,
//     // I: 'static,
// {
// }

// struct ClosureTask<I, O> {
//     // core::pin::Pin<Box<dyn core::future::Future<Output = Result<O> > + core::marker::Send+'async_trait>
// }
//
// // #[async_trait
// impl<I, O> Task<I, O> for ClosureTask<I, O> {
//     fn run(self: Box<Self>, input:I) -> Pin<Box<dyn Future<Output = Result<O>> + Send> {
//         todo!()
//     }
// }

// pub trait PinnedClosure<I, O>: FnOnce(I) -> Fut<O> + Unpin
// where
//     F: Future<Output = Result<O>>,
// {
// Running a task consumes it, which does guarantee that tasks
// may only run exactly once.
// async fn run(self: Box<Self>, input: I) -> Result<O>;
// }

// F: Future<Output = Result<O>>,

// impl<C, I, O> PinnedClosure<I, O> for C where C: std::ops::FnOnce(I) -> Fut<O> + Unpin {}

// #[cfg(feature = "off")]
mod closuretests {
    #![allow(warnings)]
    use futures::Future;

    // pub trait Closure<I, O>: FnOnce(I) -> super::Fut<O> {
    // pub trait Closure<F, I, O>: FnOnce(I) -> F {
    // pub trait Closure<F, I, O> {
    pub trait Closure<I, O> {
        fn run(self, inputs: I) -> super::Fut<O>;
    }

    // impl<F, C, I, O> Closure<F, I, O> for C
    // where
    //     C: FnOnce(I) -> F,
    //     F: Future<Output = super::Result<O>> + Send + 'static,
    // {
    //     fn run(self, inputs: I) -> super::Fut<O> {
    //         Box::pin(self(inputs))
    //     }
    // }

    pub trait Closure2<T1, T2, O> {
        fn run(self, t1: T1, t2: T2) -> super::Fut<O>;
    }

    pub trait Closure3<T1, T2, T3, O> {
        fn run(self, t1: T1, t2: T2, t3: T3) -> super::Fut<O>;
    }

    impl<F, C, T1, T2, O> Closure2<T1, T2, O> for C
    where
        C: FnOnce(T1, T2) -> F,
        F: Future<Output = super::Result<O>> + Send + 'static,
    {
        fn run(self, t1: T1, t2: T2) -> super::Fut<O> {
            let fut = self(t1, t2);
            Box::pin(fut)
        }
    }

    impl<F, C, T1, T2, T3, O> Closure3<T1, T2, T3, O> for C
    where
        C: FnOnce(T1, T2, T3) -> F,
        F: Future<Output = super::Result<O>> + Send + 'static,
    {
        fn run(self, t1: T1, t2: T2, t3: T3) -> super::Fut<O> {
            Box::pin(self(t1, t2, t3))
        }
    }

    // impl<F, C, T1, O> Closure<F, T1, O> for C
    // where
    //     // C: Closure2<F, T1, T2, O>,
    //     C: FnOnce(T1) -> F,
    //     F: Future<Output = super::Result<O>> + Send + 'static,
    //     // C: FnOnce(I) -> F,
    //     // F: Future<Output = super::Result<O>> + Send + 'static,
    // {
    //     fn run(self, inputs: T1) -> super::Fut<O> {
    //         Box::pin(self(inputs))
    //     }
    // }

    impl<C, T1, T2, O> Closure<(T1, T2), O> for C
    where
        C: Closure2<T1, T2, O>,
        // C: FnOnce(T1, T2) -> F,
        // F: Future<Output = super::Result<O>> + Send + 'static,
    {
        fn run(self, inputs: (T1, T2)) -> super::Fut<O> {
            let (t1, t2) = inputs;
            self.run(t1, t2)
            // Box::pin(self(t1, t2))
        }
    }

    impl<C, T1, T2, T3, O> Closure<(T1, T2, T3), O> for C
    where
        C: Closure3<T1, T2, T3, O>,
        // C: FnOnce(T1, T2) -> F,
        // F: Future<Output = super::Result<O>> + Send + 'static,
    {
        fn run(self, inputs: (T1, T2, T3)) -> super::Fut<O> {
            let (t1, t2, t3) = inputs;
            self.run(t1, t2, t3)
            // Box::pin(self(t1, t2))
        }
    }

    async fn test() {
        // let closure = |args: (i32, i32)| async move { Ok(args.0 + args.1) as super::Result<i32> };
        // let closure: Box<dyn Closure<_, (i32, i32), i32>> = Box::new(closure);

        let closure2 = move |t1: i32, t2: i32| async move { Ok(t1 + t2) as super::Result<i32> };

        fn assert_unpin<C, T1, T2, F>(c: &C)
        where
            C: FnOnce(T1, T2) -> F,
            F: Unpin,
        {
        }

        fn assert_closure<C>(c: &C)
        where
            C: Closure<(i32, i32), i32>,
        {
        }

        assert_closure(&closure2);
        // assert_unpin(&closure2);

        let fut = Closure::<(i32, i32), i32>::run(closure2, (0, 0));

        // let closure2box: Box<dyn Closure2<i32, i32, i32>> = Box::new(closure2);
        let closure2box = Box::new(closure2);

        let fut = Closure::<(i32, i32), i32>::run(closure2box, (0, 0));
        fut.await;
        // let fut = closure2box.run(0, 0);

        // assert_closure(closure2);
    }

    // whereFuture<Output = Result<O>> {}
}

pub(crate) enum PendingTask<I, O> {
    Task(Box<dyn Task<I, O> + Send + Sync>),
    Closure(Box<dyn Closure<I, O> + Send + Sync>),
    // Closure(Box<dyn PinnedClosure<I, O> + Unpin + Send + Sync>),
}

// #[async_trait::async_trait]
// impl<I, O> Task<I, O> for PendingTask<I, O> {
impl<I, O> PendingTask<I, O> {
    async fn run(self, input: I) -> Result<O> {
        match self {
            Self::Task(task) => task.run(input).await,
            Self::Closure(closure) => {
                // let fut = closure(input); // .boxed();
                // let fut = Box::pin(fut);
                // let fut = Pin::new(fut);
                // let fut = Pin::new(Box::new(closure(input)));

                // let fut = Box::pin(closure(input));
                // fut.await
                // let fut = Box::pin(closure.run(input));
                let closure: Box<dyn Closure<I, O>> = closure;
                // let fut = Closure::<I, O>::run(closure, input);
                // todo!();
                let fut = closure.run(input);
                fut.await
            }
        }
    }
}

// could change this to either closure or type here?
pub(crate) enum InternalState<I, O> {
    /// Task is pending and waiting to be run
    // Pending(Box<dyn Task<I, O> + Send + Sync + 'static>),
    Pending(PendingTask<I, O>),
    /// Task is running
    Running,
    /// Task succeeded with the desired output
    Succeeded(O),
    /// Task failed with an error
    #[allow(unused)]
    Failed(Box<dyn std::error::Error + Send + Sync + 'static>),
    // Failed(Arc<dyn std::error::Error + Send + Sync + 'static>),
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
    // impl CompletionResult<'_> {
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    pub fn did_succeed(&self) -> bool {
        matches!(self, Self::Succeeded)
    }

    pub fn did_fail(&self) -> bool {
        matches!(self, Self::Failed)
        // matches!(self, CompletionResult::Failed(_))
    }
}

#[async_trait::async_trait]
// pub(crate) trait Task<I, O>: std::fmt::Debug {
pub trait Task<I, O> {
    /// Running a task consumes it, which does guarantee that tasks
    /// may only run exactly once.
    async fn run(self: Box<Self>, input: I) -> Result<O>;

    fn name(&self) -> String {
        "<unnamed>".to_string()
    }
}

/// Task that is labeled.
pub trait Label<L> {
    fn label(&self) -> L;
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
/// All that this implementation does is return a shared reference to
/// the task input.
#[async_trait::async_trait]
impl<O> Task<(), O> for Input<O>
where
    O: std::fmt::Debug + Send + 'static,
{
    async fn run(self: Box<Self>, _input: ()) -> Result<O> {
        Ok(self.0)
    }

    fn name(&self) -> String {
        format!("{}", self)
    }
}

impl<O> std::fmt::Display for Input<O>
where
    O: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let short_value = summarize(&self.0, 20);
        write!(f, "Input({})", short_value)
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("task dependency failed")]
    FailedDependency,
}

/// A task node in the task graph.
///
/// The task node tracks the state of the tasks lifecycle and is assigned a unique index.
/// It is not possible to directly construct a task node to enforce correctness.
/// References to `TaskNode` can be used as dependencies.
///
/// `TaskNodes` are only generic (static) over the inputs and outputs, since that is of
/// importance for using a task node as a dependency for another task
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

    // pub fn closure<C, F>(closure: C) -> Self
    pub fn closure<C>(closure: Box<C>) -> Self
    where
        C: Closure<I, O> + Send + Sync + 'static,
        // C: Closure<F, I, O> + Send + Sync + 'static,
        // F: Future<Output = Result<O>> + Send + Sync + 'static,
        I: Send + 'static,
        // I: Send + Sync + 'static,
    {
        // todo!();
        // let boxed_closure = |inputs: I| {
        //     closure
        // }
        // fn boxed_closure(closure: Closure<F, I, O>) -> Fut<O> {
        //     closure
        // }

        // let closure = boxed_closure(closure);
        // let closure = move |inputs: I| {
        //     // box the future
        //     // Box::pin(async move { closure(inputs).await }) as Fut<O> // Box<dyn Future<Output>>
        //     Box::pin(async move { closure(inputs).await }) as Fut<O> // Box<dyn Future<Output>>
        // };
        // let future = Box::new(closure);
        // let future = Box::pin(Box::new(closure));
        // todo!();
        // let task = PendingTask::Closure(Box::new(closure));
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
    // pub short_task_name: String,
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

    // pub fn closure<F, C, D>(closure: C, deps: D, label: L, index: dag::Idx) -> Self
    // pub fn closure<C, D>(closure: Box<C>, deps: D, label: L, index: dag::Idx) -> Self
    pub fn closure<C, D>(closure: Box<C>, deps: D, label: L, index: dag::Idx) -> Self
    where
        C: Closure<I, O> + Send + Sync + 'static,
        // C: Closure<F, I, O> + Send + Sync + 'static,
        // F: Future<Output = Result<O>> + Send + Sync + 'static,
        // F: Future<Output = Result<O>>,
        // T: task::BoxedClosure<I, O> + Send + Sync + 'static,
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
        // let arguments: Vec<_> = self
        //     .dependencies()
        //     .iter()
        //     .map(|dep| dep.as_argument())
        //     .collect();
        // write!(f, "{}({})", self.task_name, arguments.join(", "))
        write!(f, "{}", self.signature())
    }
}

// impl<I, O, L> std::fmt::Debug for Node<I, O, L>
// where
//     I: std::fmt::Debug,
//     O: std::fmt::Debug,
//     L: std::fmt::Debug,
// {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("TaskNode")
//             .field("id", &self.index)
//             .field("task", &self.task_name)
//             .field("label", &self.label)
//             // safety: panics if the lock is already held by the current thread.
//             .field("state", &self.state.read().unwrap())
//             .field("dependencies", &self.dependencies)
//             .finish()
//     }
// }

#[async_trait::async_trait]
impl<I, O, L> Schedulable<L> for Node<I, O, L>
where
    I: std::fmt::Debug + Send + Sync + 'static,
    O: std::fmt::Debug + Send + Sync + 'static,
    L: std::fmt::Debug + Sync + 'static,
{
    fn succeeded(&self) -> bool {
        // safety: panics if the lock is already held by the current thread.
        let inner = self.inner.read().unwrap();
        inner.state.did_succeed()
        // matches!(*self.state.read().unwrap(), State::Succeeded(_))
    }

    // fn fail(&self, err: Arc<dyn std::error::Error + Send + Sync + 'static>) {
    fn fail(&self, err: Box<dyn std::error::Error + Send + Sync + 'static>) {
        let mut inner = self.inner.write().unwrap();
        inner.state = InternalState::Failed(err);
    }

    fn state(&self) -> State {
        // safety: panics if the lock is already held by the current thread.
        let inner = self.inner.read().unwrap();
        match inner.state {
            InternalState::Pending(_) => State::Pending,
            InternalState::Running => State::Running,
            InternalState::Succeeded(_) => State::Succeeded,
            InternalState::Failed(_) => State::Failed,
            // State::Failed(err) => CompletionResult::Failed(&err),
            // State::Failed(err) => CompletionResult::Failed(err.clone()),
        }
    }

    fn as_argument(&self) -> String {
        let inner = self.inner.read().unwrap();
        match inner.state {
            InternalState::Pending(_) => "<pending>".to_string(),
            InternalState::Running => "<running>".to_string(),
            InternalState::Succeeded(ref value) => format!("{:?}", value),
            InternalState::Failed(_) => "<failed>".to_string(),
        }
    }

    fn created_at(&self) -> Instant {
        self.created_at
    }

    fn started_at(&self) -> Option<Instant> {
        // safety: panics if the lock is already held by the current thread.
        let inner = self.inner.read().unwrap();
        inner.started_at
    }

    fn completed_at(&self) -> Option<Instant> {
        // safety: panics if the lock is already held by the current thread.
        let inner = self.inner.read().unwrap();
        inner.completed_at
    }

    fn index(&self) -> dag::Idx {
        self.index
    }

    async fn run(&self) {
        // let state = {
        //     let mut inner = self.inner.write().unwrap();
        //     if let InternalState::Pending(_) = inner.state {
        //         // set the state to running
        //         // returns owned previous value (pending task)
        //         std::mem::replace(&mut inner.state, InternalState::Running)
        //     } else {
        //         // already done
        //         return;
        //     }
        // };
        // let InternalState::Pending(task) = state else {
        //     return;
        // };

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

        // check if task was already marked as failed
        // let state = &mut *self.inner.write().unwrap();
        assert!(inner.state.is_running());
        // if !inner.state.is_running() {
        //     return;
        // }
        inner.state = match result {
            Ok(output) => InternalState::Succeeded(output),
            Err(err) => InternalState::Failed(err.into()),
        };
    }

    // async fn run(&self) -> Option<Result<O>> {
    //     // get the inputs from the dependencies
    //     let inputs = self.dependencies.inputs().unwrap();
    //     println!("running task {self:?}({inputs:?})");
    //     let state = {
    //         let state = &mut *self.state.write().unwrap();
    //         if let State::Pending(_) = state {
    //             self.started_at
    //                 .write()
    //                 .unwrap()
    //                 .get_or_insert(Instant::now());
    //
    //             // returns owned previous value (pending task)
    //             std::mem::replace(state, State::Running)
    //         } else {
    //             // already done
    //             return None;
    //         }
    //     };
    //     if let State::Pending(task) = state {
    //         // this will consume the task
    //         let result = task.run(inputs).await;
    //         Some(result);
    //
    //         // self.completed_at
    //         //     .write()
    //         //     .unwrap()
    //         //     .get_or_insert(Instant::now());
    //         //
    //         // // check if task was already marked as failed
    //         // let state = &mut *self.state.write().unwrap();
    //         // if !matches!(state, State::Running { .. }) {
    //         //     return;
    //         // }
    //         // *state = match result {
    //         //     Ok(output) => State::Succeeded(output),
    //         //     Err(err) => State::Failed(err.into()),
    //         // };
    //     }
    //     None
    // }

    fn label(&self) -> &L {
        &self.label
    }

    fn name(&self) -> &str {
        &self.task_name
        // format!("{self:?}")
    }

    // fn short_name(&self) -> String {
    //     format!("{self}")
    // }

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
        // safety: panics if the lock is already held by the current thread.
        let inner = self.inner.read().unwrap();
        match inner.state {
            InternalState::Succeeded(ref output) => Some(output.clone()),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Product<H, T: IntoTuple>(pub(crate) H, pub(crate) T);

// Converts Product (and ()) into tuples.
pub trait IntoTuple {
    type Tuple: Tuple<Product = Self>;

    fn flatten(self) -> Self::Tuple;
}

// Typeclass that tuples can be converted into a Product (or unit ()).
pub trait Tuple: Sized {
    type Product: IntoTuple<Tuple = Self>;

    fn into_product(self) -> Self::Product;

    #[inline]
    fn combine<T>(self, other: T) -> CombinedTuples<Self, T>
    where
        Self: Sized,
        T: Tuple,
        Self::Product: Combine<T::Product>,
    {
        self.into_product().combine(other.into_product()).flatten()
    }
}

pub type CombinedTuples<T, U> =
    <<<T as Tuple>::Product as Combine<<U as Tuple>::Product>>::Output as IntoTuple>::Tuple;

// Combines Product together.
pub trait Combine<T: IntoTuple> {
    type Output: IntoTuple;

    fn combine(self, other: T) -> Self::Output;
}

impl<T: IntoTuple> Combine<T> for () {
    type Output = T;
    #[inline]
    fn combine(self, other: T) -> Self::Output {
        other
    }
}

impl<H, T: IntoTuple, U: IntoTuple> Combine<U> for Product<H, T>
where
    T: Combine<U>,
    Product<H, <T as Combine<U>>::Output>: IntoTuple,
{
    type Output = Product<H, <T as Combine<U>>::Output>;

    #[inline]
    fn combine(self, other: U) -> Self::Output {
        Product(self.0, self.1.combine(other))
    }
}

impl IntoTuple for () {
    type Tuple = ();
    #[inline]
    fn flatten(self) -> Self::Tuple {}
}

impl Tuple for () {
    type Product = ();

    #[inline]
    fn into_product(self) -> Self::Product {}
}

// TODO: add documentation
macro_rules! task {
    ($name:ident: $( $type:ident ),*) => {
        #[allow(non_snake_case)]
        #[async_trait::async_trait]
        pub trait $name<$( $type ),*, O>: std::fmt::Debug {
            async fn run(self: Box<Self>, $($type: $type),*) -> Result<O>;

            fn name(&self) -> String {
                format!("{self:?}")
            }
        }

        #[allow(non_snake_case)]
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

        // #[allow(non_snake_case)]
        // #[async_trait::async_trait]
        // impl<T, $( $type ),*, O> $name<$( $type ),*, O> for T
        // where
        //     // T: $name<$( $type ),*, O> + Send + 'static,
        //     T: FnOnce($( $type ),*) -> O + std::fmt::Debug + Send + 'static,
        //     $($type: std::fmt::Debug + Send + 'static),*
        // {
        //     // async fn run(self: Box<Self>, input: ($( $type ),*,)) -> Result<O> {
        //     async fn run(self: Box<Self>, input: ($( $type ),*,)) -> Result<O> {
        //         // destructure to tuple and call
        //         let ($( $type ),*,) = input;
        //         todo!();
        //         // $name::run(self, $( $type ),*).await
        //     }
        // }


        // #[allow(non_snake_case)]
        // #[derive(Debug)]
        // pub struct $name<$( $type ),*, O> {
        //     // async fn run(self: Box<Self>, $($type: $type),*) -> Result<O>;
        //     inner: FnOnce
        // }


        // #[allow(non_snake_case)]
        // #[async_trait::async_trait]
        // impl<T, $( $type ),*, O> Task<($( $type ),*,), O> for T
        // where
        //     T: FnOnce($( $type ),*) -> O + Send + 'static,
        //     // T: $name<$( $type ),*, O> + Send + 'static,
        //     $($type: std::fmt::Debug + Send + 'static),*
        // {
        //     async fn run(self: Box<Self>, input: ($( $type ),*,)) -> Result<O> {
        //         // destructure to tuple and call
        //         // let ($( $type ),*,) = input;
        //         // $name::run(self, $( $type ),*).await
        //         todo!();
        //     }
        // }


        // impl<T, F> Task2<T> for F
        // where
        //     F: FnMut(&T) -> T,
        // {
        //     fn foo(&mut self, x: &T) -> T {
        //         self(x)
        //     }
        // }
    }
}

// trait Closure2<T1, T2, O>: FnOnce(T1, T2) -> Result<O> + Send + 'static {}

// impl<T1, T2, O> Closure2<T1, T2, O> for dyn FnOnce(T1, T2) -> O + std::fmt::Debug + Send + 'static {}

// pub struct TaskClosure2<F, T1, T2, O>
// pub struct TaskClosure2<F, T1, T2, O>
// struct Marker {}
// struct Closure2<Marker, T1, T2, O>
// // where
// // F: FnOnce(T1, T2) -> O + Send + 'static,
// {
//     // inner: F,
//     inner: Box<dyn FnOnce(T1, T2) -> O + Send + 'static>,
//     t1: std::marker::PhantomData<(T1, T2, O)>,
//     marker: std::marker::PhantomData<Marker>,
// }

// #[allow(non_snake_case)]
// #[async_trait::async_trait]
// pub trait ClosureTask2<$( $type ),*, O>: std::fmt::Debug {
//     async fn run(self: Box<Self>, $($type: $type),*) -> Result<O>;
// }

// struct Test {}
//
// impl std::ops::FnOnce<(String, String)> for Test {
//     type Output = String;
//
//     fn call_once(self, args: (String, String)) -> Self::Output {
//         "".to_string()
//     }
// }

// #[allow(non_snake_case)]
// #[async_trait::async_trait]
// // impl<C, T1, T2, O> Task2<T1, T2, O> for Closure2<T1, T2, O>
// impl<T1, T2, O> Task<(T1, T2), O> for Closure2<Marker, T1, T2, O>
// // impl<T1, T2, O> Task<(T1, T2), O> for Box<dyn Closure2<T1, T2, O>>
// where
//     // C: Closure2<T1, T2, O>,
//     // F: FnOnce(T1, T2) -> O + Send + 'static,
//     // T: TaskClosure2<T1, T2, O>,
//     // T: FnOnce(T1, T2) -> O + Send + 'static,
//     // T: $name<$( $type ),*, O> + Send + 'static,
//     T1: std::fmt::Debug + Send + 'static,
//     T2: std::fmt::Debug + Send + 'static,
//     O: std::fmt::Debug + Send + 'static,
// {
//     async fn run(self: Box<Self>, input: (T1, T2)) -> Result<O> {
//         // destructure to tuple and call
//         // let ($( $type ),*,) = input;
//         // $name::run(self, $( $type ),*).await
//         let (t1, t2) = input;
//         todo!();
//         // self.call(t1, t2).await
//         // <self as FnOnce>::call_once(self, (t1, t2))
//         // self(t1, t2)
//     }
// }

// #[allow(non_snake_case)]
// #[async_trait::async_trait]
// impl<C, T1, T2, O> Task<(T1, T2), O> for C
// // impl<T1, T2, O> Task<(T1, T2), O> for Box<dyn Closure2<T1, T2, O>>
// where
//     C: Closure2<T1, T2, O>,
//     // F: FnOnce(T1, T2) -> O + Send + 'static,
//     // T: TaskClosure2<T1, T2, O>,
//     // T: FnOnce(T1, T2) -> O + Send + 'static,
//     // T: $name<$( $type ),*, O> + Send + 'static,
//     T1: std::fmt::Debug + Send + 'static,
//     T2: std::fmt::Debug + Send + 'static,
//     O: std::fmt::Debug + Send + 'static,
// {
//     async fn run(self: Box<Self>, input: (T1, T2)) -> Result<O> {
//         // destructure to tuple and call
//         // let ($( $type ),*,) = input;
//         // $name::run(self, $( $type ),*).await
//         // todo!();
//         let (t1, t2) = input;
//         // self.call(t1, t2).await
//         // <self as FnOnce>::call_once(self, (t1, t2))
//         self(t1, t2)
//     }
// }

macro_rules! product {
    ($H:expr) => { Product($H, ()) };
    ($H:expr, $($T:expr),*) => { Product($H, product!($($T),*)) };
}

macro_rules! Product {
    ($H:ty) => { Product<$H, ()> };
    ($H:ty, $($T:ty),*) => { Product<$H, Product!($($T),*)> };
}

macro_rules! product_pat {
    ($H:pat) => { Product($H, ()) };
    ($H:pat, $($T:pat),*) => { Product($H, product_pat!($($T),*)) };
}

macro_rules! generics {
    ($type:ident) => {
        impl<$type> IntoTuple for Product!($type) {
            type Tuple = ($type,);

            #[inline]
            fn flatten(self) -> Self::Tuple {
                (self.0,)
            }
        }

        impl<$type> Tuple for ($type,) {
            type Product = Product!($type);
            #[inline]
            fn into_product(self) -> Self::Product {
                product!(self.0)
            }
        }
    };


    ($type1:ident, $( $type:ident ),*) => {
        generics!($( $type ),*);

        impl<$type1, $( $type ),*> IntoTuple for Product!($type1, $($type),*) {
            type Tuple = ($type1, $( $type ),*);

            #[inline]
            fn flatten(self) -> Self::Tuple {
                #[allow(non_snake_case)]
                let product_pat!($type1, $( $type ),*) = self;
                ($type1, $( $type ),*)
            }
        }

        impl<$type1, $( $type ),*> Tuple for ($type1, $($type),*) {
            type Product = Product!($type1, $( $type ),*);

            #[inline]
            fn into_product(self) -> Self::Product {
                #[allow(non_snake_case)]
                let ($type1, $( $type ),*) = self;
                product!($type1, $( $type ),*)
            }
        }
    };
}

// #[allow(non_snake_case)]
//         #[async_trait::async_trait]
//         impl<T, $( $type ),*, O> Task<($( $type ),*,), O> for T
//         where
//             T: $name<$( $type ),*, O> + Send + 'static,
//             $($type: std::fmt::Debug + Send + 'static),*
//         {
//             async fn run(self: Box<Self>, input: ($( $type ),*,)) -> Result<O> {
//                 // destructure to tuple and call
//                 let ($( $type ),*,) = input;
//                 $name::run(self, $( $type ),*).await
//             }
//         }

task!(Task1: T1);
task!(Task2: T1, T2);
task!(Task3: T1, T2, T3);

generics! {
    T1,
    T2,
    T3
    // T4,
    // T5,
    // T6,
    // T7,
    // T8,
    // T9,
    // T10,
    // T11,
    // T12,
    // T13,
    // T14,
    // T15,
    // T16
}

// TODO: add documentation
macro_rules! closure {
    ($name:ident: $( $type:ident ),*) => {
        #[allow(non_snake_case)]
        #[async_trait::async_trait]
        // pub trait $name<F, $( $type ),*, O> {
        pub trait $name<$( $type ),*, O> {
            // fn run(self, $($type: $type),*) -> F;
            // fn run(self, $($type: $type),*) -> Fut<O>;
            fn run(self: Box<Self>, $($type: $type),*) -> Fut<O>;
        }

        #[allow(non_snake_case)]
        #[async_trait::async_trait]
        // impl<F, C, $( $type ),*, O> $name<F, $( $type ),*, O> for C
        // impl<F, C, $( $type ),*, O> $name<F, $( $type ),*, O> for C
        impl<F, C, $( $type ),*, O> $name<$( $type ),*, O> for C
        where
            C: FnOnce($( $type ),*) -> F,
            // F: Future<Output = Result<O>> + Send + 'static,
            F: Future<Output = Result<O>> + Send + 'static,
        {
            fn run(self: Box<Self>, $($type: $type),*) -> Fut<O> {
            // fn run(self, $($type: $type),*) -> Fut<O> {
            // fn run(self, $($type: $type),*) -> F {
                Box::pin(self($( $type ),*))
                // self($( $type ),*)
            }
        }

        #[allow(non_snake_case)]
        #[async_trait::async_trait]
        // impl<F, C, $( $type ),*, O> Closure<($( $type ),*,), O> for C
        impl<C, $( $type ),*, O> Closure<($( $type ),*,), O> for C
        where
            C: $name<$( $type ),*, O>,
            // C: $name<F, $( $type ),*, O>,
            // F: Future<Output = Result<O>> + Unpin + Send + 'static,
        {
            //
            fn run(self: Box<Self>, input: ($( $type ),*,)) -> Fut<O> {
            // fn run(self, input: ($( $type ),*,)) -> Fut<O> {
                // destructure to tuple and call
                let ($( $type ),*,) = input;
                let fut = C::run(self, $( $type ),*);
                // let fut = $name::run(self, $( $type ),*);
                // Box::pin(fut)
                fut
            }
        }
    }
}

closure!(Closure1: T1);
closure!(Closure2: T1, T2);
closure!(Closure3: T1, T2, T3);
