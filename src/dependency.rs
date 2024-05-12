use crate::schedule::Schedulable;

use std::sync::Arc;

pub trait Dependency<O, L>: Schedulable<L> {
    fn output(&self) -> Option<O>;
}

pub trait Dependencies<O, L> {
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable<L>>>;
    fn inputs(&self) -> Option<O>;
}

// no dependencies
impl<L> Dependencies<(), L> for () {
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable<L>>> {
        vec![]
    }

    fn inputs(&self) -> Option<()> {
        Some(())
    }
}

// 1 dependency
impl<D1, T1, L> Dependencies<(T1,), L> for (Arc<D1>,)
where
    D1: Dependency<T1, L> + 'static,
    T1: Clone,
{
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable<L>>> {
        vec![self.0.clone() as Arc<dyn Schedulable<L>>]
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
impl<D1, D2, T1, T2, L> Dependencies<(T1, T2), L> for (Arc<D1>, Arc<D2>)
where
    D1: Dependency<T1, L> + 'static,
    D2: Dependency<T2, L> + 'static,
    T1: Clone,
    T2: Clone,
{
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable<L>>> {
        vec![
            self.0.clone() as Arc<dyn Schedulable<L>>,
            self.1.clone() as Arc<dyn Schedulable<L>>,
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

impl<O, L> std::fmt::Debug for dyn Dependencies<O, L> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}",
            self.to_vec().iter().map(|d| d.index()).collect::<Vec<_>>()
        )
    }
}

impl<O, L> std::fmt::Debug for dyn Dependencies<O, L> + Send + Sync + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}",
            self.to_vec().iter().map(|d| d.index()).collect::<Vec<_>>()
        )
    }
}
