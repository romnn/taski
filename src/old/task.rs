use super::error::{Error, TaskError};
use async_trait::async_trait;
use downcast_rs::{impl_downcast, Downcast};
use std::collections::hash_map::{Entry, HashMap};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;

// add dependencies:
// lets say: outputs artwork image data
// idea: for all the Tasks, make them return the same outcome type
// then, each Task with prerequisits can get them as a vector of those outcomes

pub type TaskFun<C, I, O, E> = Box<
    dyn FnOnce(C, HashMap<I, O>) -> Pin<Box<dyn Future<Output = Result<O, E>> + Send + Sync>>
        + Send
        + Sync,
>;

/// a task
pub struct Task<I, C, O, E> {
    /// unique identifier for this task
    pub id: I,
    /// task function
    pub task: TaskFun<C, I, O, E>,
}

impl<I, C, O, E> Task<I, C, O, E>
where
    I: Clone,
{
    pub fn id(&self) -> I {
        self.id.clone()
    }
}

/// state of a task during execution
pub enum State<I, C, O, E>
where
    I: Clone + std::fmt::Debug,
    E: Clone + std::fmt::Debug,
{
    /// task is waiting to be executed
    Pending(TaskFun<C, I, O, E>),
    /// task is executed
    Running,
    /// task succeeded
    Success(O),
    /// task failed
    Failed(TaskError<I, E>),
}

impl<I, C, O, E> State<I, C, O, E>
where
    I: Clone + std::fmt::Debug,
    E: Clone + std::fmt::Debug,
    O: Clone,
{
    pub fn succeeded(&self) -> bool {
        match self {
            State::Success(_) => true,
            _ => false,
        }
    }

    pub fn success(self) -> Option<O> {
        match self {
            State::Success(res) => Some(res),
            _ => None,
        }
    }

    pub fn error(self) -> Option<TaskError<I, E>> {
        match self {
            State::Failed(err) => Some(err),
            _ => None,
        }
    }
}

/// a collection of tasks during execution
pub struct Tasks<I, C, O, E>
where
    I: Clone + std::fmt::Debug,
    E: Clone + std::fmt::Debug,
    O: Clone,
{
    inner: HashMap<I, State<I, C, O, E>>,
}

impl<I, C, O, E> Tasks<I, C, O, E>
where
    I: Clone + std::fmt::Debug,
    E: Clone + std::fmt::Debug,
    O: Clone,
{
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// iterator over all succeeded tasks
    pub fn succeeded(&self) -> impl Iterator<Item = (&I, &O)> {
        self.inner.iter().filter_map(|(id, state)| match state {
            State::Success(res) => Some((id, res)),
            _ => None,
        })
    }

    /// iterator over all failed tasks
    pub fn failed(&self) -> impl Iterator<Item = (&I, &TaskError<I, E>)> {
        self.inner.iter().filter_map(|(id, state)| match state {
            State::Failed(err) => Some((id, err)),
            _ => None,
        })
    }

    /// iterator over all pending tasks
    pub fn pending(&self) -> impl Iterator<Item = (&I, &TaskFun<C, I, O, E>)> {
        self.inner.iter().filter_map(|(id, state)| match state {
            State::Pending(task) => Some((id, task)),
            _ => None,
        })
    }

    /// iterator over all running tasks
    pub fn running(&self) -> impl Iterator<Item = &I> {
        self.inner.iter().filter_map(|(id, state)| match state {
            State::Running => Some(id),
            _ => None,
        })
    }
}

impl<I, C, O, E> Deref for Tasks<I, C, O, E>
where
    I: Clone + std::fmt::Debug,
    E: Clone + std::fmt::Debug,
    O: Clone,
{
    type Target = HashMap<I, State<I, C, O, E>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<I, C, O, E> DerefMut for Tasks<I, C, O, E>
where
    I: Clone + std::fmt::Debug,
    E: Clone + std::fmt::Debug,
    O: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// task node with dependencies
pub struct TaskNode<I, C, O, E> {
    // pub task: TaskFun<C, O, E>,
    pub task: Task<I, C, O, E>,
    pub dependencies: Vec<Box<dyn IntoTask<I, C, O, E>>>,
}

// todo: make this a normal into trait?
#[async_trait]
pub trait IntoTask<I, C, O, E> {
    // pub trait IntoTask<I, L, C, O, E> {
    // : Downcast {
    // fn dependencies(&self) -> Vec<Dependency> {
    //     // let mut dep = Dependency::new(&self);
    // }
    // fn id(&self) -> I;
    //     // must return a unique id for each task here

    // }
    // pub id: I,
    //     pub labels: Vec<L>,
    //     pub dependencies: Vec<Box<dyn IntoTask<I, L, C, O, E>>>,
    //     pub task: TaskFun<C, O, E>,

    // fn id(&self) -> I;
    // fn labels(&self) -> &Vec<L>;
    // fn dependencies(&self) -> &Vec<Box<dyn Task<I, L, C, O, E>>>;
    // async fn task(
    //     self,
    //     ctx: C,
    //     prereqs: Vec<O>,
    // ) -> Pin<Box<dyn Future<Output = Result<O, E>> + Send + Sync>>;
    // Task<I, L, C, O, E>;

    // fn into_task(self: Box<Self>) -> TaskNode<I, L, C, O, E>;
    fn into_task(self: Box<Self>) -> TaskNode<I, C, O, E>;
    // fn into_task(self: Self) -> TaskNode<I, L, C, O, E>;

    // fn plan(&self, plan: &mut PlanBuilder<C, O, E>) -> Result<(), Error<E>> {
    //     // this is the default trait implementation
    //     #![allow(unused_variables)]

    //     Ok(())
    // }
}

// impl_downcast!(IntoTask<C, O, E>);
