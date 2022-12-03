use std::collections::{HashMap, HashSet};

#[derive(thiserror::Error, Clone, Debug)]
pub enum TaskError<I, E>
where
    I: Clone + std::fmt::Debug,
    E: Clone + std::fmt::Debug,
{
    #[error("task failed: `{0:?}`")]
    Failed(E),

    #[error("task not found: `{0:?}`")]
    NoTask(I),

    #[error("preconditions not met: `{0:?}`")]
    Precondition(I),
}

#[derive(thiserror::Error, Debug)]
pub enum Error<E, I>
where
    I: Clone + std::fmt::Debug,
    E: Clone + std::fmt::Debug,
{
    #[error("invalid configuration: `{0}`")]
    InvalidConfiguration(String),

    #[error("some tasks failed")]
    Failed(HashMap<I, TaskError<I, E>>),

    #[error("schedule error: `{0}`")]
    Schedule(#[from] ScheduleError<I>),
}

#[derive(thiserror::Error, Debug)]
pub enum ScheduleError<I>
where
    I: std::fmt::Debug,
{
    #[error("dependency cycle detected")]
    Cycle,

    #[error("new dependencies: `{0:?}`")]
    NewDependencies(HashSet<I>),

    #[error("policy scheduled task that is not ready: `{0:?}`")]
    BadPolicy(I),
}
