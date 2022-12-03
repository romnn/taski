use super::error::{Error, ScheduleError};
use super::task::{IntoTask, State, Task, TaskNode, Tasks};
use super::{Schedule, Scheduler};
use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use std::cell::RefCell;
use std::cmp::Eq;
use std::collections::hash_map::{Entry, HashMap};
use std::collections::HashSet;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::RwLock;

#[async_trait]
pub trait Policy {
    async fn arbitrate<I, C, O, E>(
        &self,
        tasks: &Tasks<I, C, O, E>,
        scheduler: &Schedule<I>,
    ) -> Option<I>
    where
        I: Clone + Send + Sync + Eq + Hash + std::fmt::Debug + 'static,
        C: Send + Sync + 'static,
        O: Clone + Send + Sync + 'static,
        E: Clone + Send + Sync + std::fmt::Debug + 'static;
}

pub struct GreedyPolicy {
    max_tasks: Option<usize>,
}

impl GreedyPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_tasks(max_tasks: Option<usize>) -> Self {
        Self { max_tasks }
    }
}

impl Default for GreedyPolicy {
    fn default() -> Self {
        Self { max_tasks: Some(num_cpus::get()) }
    }
}

#[async_trait]
impl Policy for GreedyPolicy {
    async fn arbitrate<I, C, O, E>(
        &self,
        tasks: &Tasks<I, C, O, E>,
        schedule: &Schedule<I>,
    ) -> Option<I>
    where
        I: Clone + Send + Sync + Eq + Hash + std::fmt::Debug + 'static,
        C: Send + Sync + 'static,
        O: Clone + Send + Sync + 'static,
        E: Clone + Send + Sync + std::fmt::Debug + 'static,
    {
        // returning some id -> task with id will be executed
        // returning None -> no task can be scheduled at the moment, wait for any task tofinish
        // before attempting to schedule again

        // take any ready job unless the max number of jobs is exceeded
        if let Some(max_tasks) = self.max_tasks {
            if tasks.running().count() >= max_tasks {
                return None;
            }
        }
        schedule.ready().cloned().next()
    }
}
