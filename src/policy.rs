use crate::schedule::Schedule;
use crate::{dag, task};
// use crate::{executor::Executor, task};

pub trait Policy<L> {
    // fn arbitrate(&self, schedule: &dyn Executor<L>) -> Option<task::Ref<L>>;
    fn arbitrate(&self, ready: &[dag::Idx], schedule: &Schedule<L>) -> Option<dag::Idx>;
}

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fifo {
    max_tasks: Option<usize>,
}

impl Fifo {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn max_tasks(max_tasks: Option<usize>) -> Self {
        Self { max_tasks }
    }
}

impl<L> Policy<L> for Fifo {
    // fn arbitrate(&self, schedule: &dyn Executor<L>) -> Option<task::Ref<L>> {
    fn arbitrate(&self, ready: &[dag::Idx], schedule: &Schedule<L>) -> Option<dag::Idx> {
        if let Some(limit) = self.max_tasks {
            if schedule.running().count() >= limit {
                // do not schedule new task
                return None;
            }
        }
        // schedule first task in the ready queue
        ready.iter().next().copied()
    }
}

// #[derive(Debug, Default, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
// pub struct Priority {
//     max_tasks: Option<usize>,
// }
//
// impl Priority {
//     #[must_use]
//     pub fn new() -> Self {
//         Self::default()
//     }
//
//     #[must_use]
//     pub fn max_tasks(max_tasks: Option<usize>) -> Self {
//         Self { max_tasks }
//     }
// }
//
// impl<L> Policy<L> for Priority
// where
//     L: std::cmp::Ord,
// {
//     fn arbitrate(&self, schedule: &dyn Executor<L>) -> Option<task::Ref<L>> {
//         if let Some(limit) = self.max_tasks {
//             if schedule.running().len() >= limit {
//                 // do not schedule new task
//                 return None;
//             }
//         }
//         // schedule highest priority task from the ready queue
//         let mut ready: Vec<_> = schedule.ready().collect();
//         ready.sort_by(|a, b| a.label().cmp(b.label()));
//         ready.first().cloned()
//     }
// }
