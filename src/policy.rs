use crate::{dag, execution::Execution, schedule::Schedule, task};

use std::collections::VecDeque;

pub trait Policy<'id, L> {
    fn reset(&mut self);

    fn on_task_ready(&mut self, task_id: dag::TaskId<'id>, schedule: &Schedule<'id, L>);

    fn on_task_started(&mut self, task_id: dag::TaskId<'id>, schedule: &Schedule<'id, L>);

    fn on_task_finished(
        &mut self,
        task_id: dag::TaskId<'id>,
        state: task::State,
        schedule: &Schedule<'id, L>,
    );

    fn next_task(
        &mut self,
        schedule: &Schedule<'id, L>,
        execution: &Execution<'id>,
    ) -> Option<dag::TaskId<'id>>;
}

#[derive(Debug, Default, Clone)]
pub struct Fifo {
    pub max_concurrent: Option<usize>,
    ready: VecDeque<dag::Idx>,
}

impl Fifo {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn max_concurrent(limit: Option<usize>) -> Self {
        Self::from_limit(limit)
    }

    #[must_use]
    pub fn from_limit(limit: Option<usize>) -> Self {
        Self {
            max_concurrent: limit,
            ready: VecDeque::new(),
        }
    }
}

impl<'id, L: 'id> Policy<'id, L> for Fifo {
    fn reset(&mut self) {
        self.ready.clear();
    }

    fn on_task_ready(&mut self, task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {
        self.ready.push_back(task_id.idx());
    }

    fn on_task_started(&mut self, _task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {}

    fn on_task_finished(
        &mut self,
        _task_id: dag::TaskId<'id>,
        _state: task::State,
        _schedule: &Schedule<'id, L>,
    ) {
    }

    fn next_task(
        &mut self,
        schedule: &Schedule<'id, L>,
        execution: &Execution<'id>,
    ) -> Option<dag::TaskId<'id>> {
        if let Some(limit) = self.max_concurrent {
            if execution.running_count() >= limit {
                return None;
            }
        }

        while let Some(task_idx) = self.ready.pop_front() {
            let task_id = schedule.task_id(task_idx);
            if execution.state(task_id).is_pending() {
                return Some(task_id);
            }
        }
        None
    }
}

#[derive(Debug, Default, Clone)]
pub struct Priority {
    pub max_concurrent: Option<usize>,
    ready: Vec<dag::Idx>,
}

impl Priority {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn max_concurrent(limit: Option<usize>) -> Self {
        Self::from_limit(limit)
    }

    #[must_use]
    pub fn from_limit(limit: Option<usize>) -> Self {
        Self {
            max_concurrent: limit,
            ready: Vec::new(),
        }
    }
}

impl<'id, L> Policy<'id, L> for Priority
where
    L: std::cmp::Ord + 'id,
{
    fn reset(&mut self) {
        self.ready.clear();
    }

    fn on_task_ready(&mut self, task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {
        self.ready.push(task_id.idx());
    }

    fn on_task_started(&mut self, _task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {}

    fn on_task_finished(
        &mut self,
        _task_id: dag::TaskId<'id>,
        _state: task::State,
        _schedule: &Schedule<'id, L>,
    ) {
    }

    fn next_task(
        &mut self,
        schedule: &Schedule<'id, L>,
        execution: &Execution<'id>,
    ) -> Option<dag::TaskId<'id>> {
        if let Some(limit) = self.max_concurrent {
            if execution.running_count() >= limit {
                return None;
            }
        }

        let mut best: Option<(usize, &L)> = None;
        for (idx, &task_idx) in self.ready.iter().enumerate() {
            let task_id = schedule.task_id(task_idx);
            if !execution.state(task_id).is_pending() {
                continue;
            }

            let label = schedule.task_label(task_id);
            match best {
                None => best = Some((idx, label)),
                Some((best_idx, best_label)) => {
                    if label > best_label || (label == best_label && idx > best_idx) {
                        best = Some((idx, label));
                    }
                }
            }
        }

        let (best_idx, _) = best?;
        Some(schedule.task_id(self.ready.remove(best_idx)))
    }
}
