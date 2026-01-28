use crate::{dag, task};

use std::any::Any;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug)]
pub struct TaskExecution {
    pub state: task::State,
    pub started_at: Option<Instant>,
    pub completed_at: Option<Instant>,
    pub output: Option<Arc<dyn Any + Send + Sync>>,
    pub error: Option<task::Error>,
}

impl Default for TaskExecution {
    fn default() -> Self {
        Self {
            state: task::State::Pending,
            started_at: None,
            completed_at: None,
            output: None,
            error: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Execution<'id> {
    schedule_id: u64,
    tasks: Vec<TaskExecution>,
    running_count: usize,
    unfinished_count: usize,
    _phantom: std::marker::PhantomData<fn(&'id ()) -> &'id ()>,
}

impl<'id> Execution<'id> {
    #[must_use]
    pub(crate) fn new(schedule_id: u64, task_count: usize) -> Self {
        Self {
            schedule_id,
            tasks: (0..task_count).map(|_| TaskExecution::default()).collect(),
            running_count: 0,
            unfinished_count: task_count,
            _phantom: std::marker::PhantomData,
        }
    }

    fn validate_task_id(&self, task_id: dag::TaskId<'id>) -> bool {
        if task_id.schedule_id() != self.schedule_id {
            log::error!("task id belongs to a different schedule");
            return false;
        }
        true
    }

    pub(crate) fn mark_running(&mut self, task_id: dag::TaskId<'id>, now: Instant) {
        if !self.validate_task_id(task_id) {
            return;
        }
        let Some(task) = self.tasks.get_mut(task_id.idx().index()) else {
            log::error!("invalid task id");
            return;
        };

        if task.state.is_running() {
            return;
        }

        if task.state.is_pending() {
            self.running_count = self.running_count.saturating_add(1);
        }

        task.started_at.get_or_insert(now);
        task.state = task::State::Running;
    }

    pub(crate) fn mark_succeeded(
        &mut self,
        task_id: dag::TaskId<'id>,
        now: Instant,
        output: Arc<dyn Any + Send + Sync>,
    ) {
        if !self.validate_task_id(task_id) {
            return;
        }
        let Some(task) = self.tasks.get_mut(task_id.idx().index()) else {
            log::error!("invalid task id");
            return;
        };

        if task.state.is_pending() || task.state.is_running() {
            if task.state.is_running() {
                self.running_count = self.running_count.saturating_sub(1);
            }
            self.unfinished_count = self.unfinished_count.saturating_sub(1);
        }

        task.completed_at.get_or_insert(now);
        task.output = Some(output);
        task.error = None;
        task.state = task::State::Succeeded;
    }

    pub(crate) fn mark_failed(&mut self, task_id: dag::TaskId<'id>, now: Instant, err: task::Error) {
        if !self.validate_task_id(task_id) {
            return;
        }
        let Some(task) = self.tasks.get_mut(task_id.idx().index()) else {
            log::error!("invalid task id");
            return;
        };

        if task.state.is_pending() || task.state.is_running() {
            if task.state.is_running() {
                self.running_count = self.running_count.saturating_sub(1);
            }
            self.unfinished_count = self.unfinished_count.saturating_sub(1);
        }

        task.completed_at.get_or_insert(now);
        task.output = None;
        task.error = Some(err);
        task.state = task::State::Failed;
    }

    #[must_use]
    pub fn running_count(&self) -> usize {
        self.running_count
    }

    #[must_use]
    pub fn unfinished_count(&self) -> usize {
        self.unfinished_count
    }

    #[must_use]
    pub fn state(&self, task_id: dag::TaskId<'id>) -> task::State {
        if !self.validate_task_id(task_id) {
            return task::State::Pending;
        }
        self.tasks
            .get(task_id.idx().index())
            .map(|t| t.state.clone())
            .unwrap_or(task::State::Pending)
    }

    #[must_use]
    pub(crate) fn output_ref_task_id<O: 'static>(&self, task_id: dag::TaskId<'id>) -> Option<&O> {
        if !self.validate_task_id(task_id) {
            return None;
        }
        let task = self.tasks.get(task_id.idx().index())?;
        let output = task.output.as_ref()?;
        output.downcast_ref::<O>()
    }

    #[must_use]
    pub fn output_ref<O: 'static>(&self, handle: dag::Handle<'id, O>) -> Option<&O> {
        self.output_ref_task_id::<O>(handle.task_id())
    }

    #[must_use]
    pub fn started_at(&self, task_id: dag::TaskId<'id>) -> Option<Instant> {
        if !self.validate_task_id(task_id) {
            return None;
        }
        self.tasks.get(task_id.idx().index())?.started_at
    }

    #[must_use]
    pub fn completed_at(&self, task_id: dag::TaskId<'id>) -> Option<Instant> {
        if !self.validate_task_id(task_id) {
            return None;
        }
        self.tasks.get(task_id.idx().index())?.completed_at
    }
}
