use crate::{dag, schedule::Schedule};

pub trait Policy<'id, L> {
    fn arbitrate(
        &self,
        ready: &[dag::TaskId<'id>],
        schedule: &Schedule<'id, L>,
    ) -> Option<dag::TaskId<'id>>;
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fifo {
    pub max_concurrent: Option<usize>,
}

impl Fifo {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn max_concurrent(limit: Option<usize>) -> Self {
        Self {
            max_concurrent: limit,
        }
    }
}

impl<'id, L: 'id> Policy<'id, L> for Fifo {
    fn arbitrate(
        &self,
        ready: &[dag::TaskId<'id>],
        schedule: &Schedule<'id, L>,
    ) -> Option<dag::TaskId<'id>> {
        if let Some(limit) = self.max_concurrent {
            if schedule.running().count() >= limit {
                // do not schedule new task
                return None;
            }
        }
        // schedule first task in the ready queue
        ready.iter().next().copied()
    }
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority {
    pub max_concurrent: Option<usize>,
}

impl Priority {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn max_concurrent(limit: Option<usize>) -> Self {
        Self {
            max_concurrent: limit,
        }
    }
}

impl<'id, L> Policy<'id, L> for Priority
where
    L: std::cmp::Ord + 'id,
{
    fn arbitrate(
        &self,
        ready: &[dag::TaskId<'id>],
        schedule: &Schedule<'id, L>,
    ) -> Option<dag::TaskId<'id>> {
        if let Some(limit) = self.max_concurrent {
            if schedule.running().count() >= limit {
                // do not schedule new task
                return None;
            }
        }
        // schedule highest priority task from the ready queue
        ready
            .iter()
            .max_by_key(|&&idx| schedule.dag[idx.idx()].label())
            .copied()
    }
}
