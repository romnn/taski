use crate::schedule::Schedulable;

use std::sync::Arc;

pub trait Dependency<'id, O, L: 'id>: Schedulable<'id, L> {
    fn output(&self) -> Option<O>;
}

pub trait Dependencies<'id, O, L: 'id> {
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable<'id, L> + 'id>>;
    fn inputs(&self) -> Option<O>;
}

// no dependencies
impl<'id, L: 'id> Dependencies<'id, (), L> for () {
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable<'id, L> + 'id>> {
        vec![]
    }

    fn inputs(&self) -> Option<()> {
        Some(())
    }
}

// 1 dependency
impl<'id, D1, T1, L: 'id> Dependencies<'id, (T1,), L> for (Arc<D1>,)
where
    D1: Dependency<'id, T1, L> + 'id,
    T1: Clone,
{
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable<'id, L> + 'id>> {
        vec![self.0.clone() as Arc<dyn Schedulable<'id, L> + 'id>]
    }

    fn inputs(&self) -> Option<(T1,)> {
        let (i1,) = self;
        match (i1.output(),) {
            (Some(i1),) => Some((i1,)),
            _ => None,
        }
    }
}

// two dependencies
impl<'id, D1, D2, T1, T2, L: 'id> Dependencies<'id, (T1, T2), L> for (Arc<D1>, Arc<D2>)
where
    D1: Dependency<'id, T1, L> + 'id,
    D2: Dependency<'id, T2, L> + 'id,
    T1: Clone,
    T2: Clone,
{
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable<'id, L> + 'id>> {
        vec![
            self.0.clone() as Arc<dyn Schedulable<'id, L> + 'id>,
            self.1.clone() as Arc<dyn Schedulable<'id, L> + 'id>,
        ]
    }

    fn inputs(&self) -> Option<(T1, T2)> {
        let (i1, i2) = self;
        match (i1.output(), i2.output()) {
            (Some(i1), Some(i2)) => Some((i1, i2)),
            _ => None,
        }
    }
}

impl<'id, O, L: 'id> std::fmt::Debug for dyn Dependencies<'id, O, L> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}",
            self.to_vec().iter().map(|d| d.index()).collect::<Vec<_>>()
        )
    }
}
