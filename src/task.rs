use crate::{
    dependency::{Dependencies, Dependency},
    schedule::Schedulable,
};

use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::Instant;

fn summarize(s: impl AsRef<str>, max_length: usize) -> String {
    let s = s.as_ref();
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

#[derive(Debug)]
pub enum State<I, O> {
    /// Task is pending and waiting to be run
    Pending(Box<dyn Task<I, O> + Send + Sync + 'static>),
    /// Task is running
    Running,
    /// Task succeeded with the desired output
    Succeeded(O),
    /// Task failed with an error
    Failed(Arc<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Clone)]
pub enum CompletionResult {
    /// Task is pending
    Pending,
    /// Task is running
    Running,
    /// Task succeeded
    Succeeded,
    /// Task failed with an error
    Failed(Arc<dyn std::error::Error + Send + Sync + 'static>),
}

#[async_trait::async_trait]
pub trait Task<I, O>: std::fmt::Debug {
    /// Running a task consumes it, which does guarantee that tasks
    /// may only run exactly once.
    async fn run(self: Box<Self>, input: I) -> Result<O>;

    fn name(&self) -> String {
        format!("{self:?}")
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
}

impl<O> std::fmt::Display for Input<O>
where
    O: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let short_value = summarize(format!("{}", &self.0), 20);
        f.debug_tuple("TaskInput").field(&short_value).finish()
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
#[derive(Debug)]
pub struct Node<I, O, L> {
    pub task_name: String,
    pub short_task_name: String,
    pub label: L,
    pub created_at: Instant,
    pub started_at: RwLock<Option<Instant>>,
    pub completed_at: RwLock<Option<Instant>>,
    pub state: RwLock<State<I, O>>,
    pub dependencies: Box<dyn Dependencies<I, L> + Send + Sync>,
    pub index: usize,
}

impl<I, O, L> Hash for Node<I, O, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // hash the index
        self.index.hash(state);
    }
}

impl<I, O, L> std::fmt::Display for Node<I, O, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_task_name)
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
        matches!(*self.state.read().unwrap(), State::Succeeded(_))
    }

    fn fail(&self, err: Arc<dyn std::error::Error + Send + Sync + 'static>) {
        *self.state.write().unwrap() = State::Failed(err);
    }

    fn state(&self) -> CompletionResult {
        // safety: panics if the lock is already held by the current thread.
        match &*self.state.read().unwrap() {
            State::Pending(_) => CompletionResult::Pending,
            State::Running => CompletionResult::Running,
            State::Succeeded(_) => CompletionResult::Succeeded,
            State::Failed(err) => CompletionResult::Failed(err.clone()),
        }
    }

    fn created_at(&self) -> Instant {
        self.created_at
    }

    fn started_at(&self) -> Option<Instant> {
        // safety: panics if the lock is already held by the current thread.
        *self.started_at.read().unwrap()
    }

    fn completed_at(&self) -> Option<Instant> {
        // safety: panics if the lock is already held by the current thread.
        *self.completed_at.read().unwrap()
    }

    fn index(&self) -> usize {
        self.index
    }

    async fn run(&self) {
        // get the inputs from the dependencies
        let inputs = self.dependencies.inputs().unwrap();
        println!("running task {self:?}({inputs:?})");
        let state = {
            let state = &mut *self.state.write().unwrap();
            if let State::Pending(_) = state {
                self.started_at
                    .write()
                    .unwrap()
                    .get_or_insert(Instant::now());

                // returns owned previous value (pending task)
                std::mem::replace(state, State::Running)
            } else {
                // already done
                return;
            }
        };
        if let State::Pending(task) = state {
            // this will consume the task
            let result = task.run(inputs).await;

            self.completed_at
                .write()
                .unwrap()
                .get_or_insert(Instant::now());

            // check if task was already marked as failed
            let state = &mut *self.state.write().unwrap();
            if !matches!(state, State::Running { .. }) {
                return;
            }
            *state = match result {
                Ok(output) => State::Succeeded(output),
                Err(err) => State::Failed(err.into()),
            };
        }
    }

    fn label(&self) -> &L {
        &self.label
    }

    fn name(&self) -> String {
        format!("{self:?}")
    }

    fn short_name(&self) -> String {
        format!("{self}")
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
        // safety: panics if the lock is already held by the current thread.
        match &*self.state.read().unwrap() {
            State::Succeeded(output) => Some(output.clone()),
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

macro_rules! task {
    ($name:ident: $( $type:ident ),*) => {
        #[allow(non_snake_case)]
        #[async_trait::async_trait]
        pub trait $name<$( $type ),*, O>: std::fmt::Debug {
            async fn run(self: Box<Self>, $($type: $type),*) -> Result<O>;
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
        }
    }
}

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
