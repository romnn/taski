//! Dependency extraction for task inputs.
//!
//! When you add a task to a [`crate::Schedule`], you specify its dependencies. A dependency value
//! must implement [`Dependencies`]. The executor uses it to:
//! - enumerate prerequisite tasks (`task_ids`)
//! - construct a typed input value from an [`Execution`] (`inputs`)
//!
//! The crate provides [`Dependencies`] implementations for tuples of [`dag::Handle`] up to arity 8
//! (matching the `TaskN` / `ClosureN` families).

use crate::{dag, execution::Execution};

/// Typed dependencies for a task.
///
/// Most users use the built-in tuple implementations, e.g. `(a, b)` where `a` and `b` are
/// [`dag::Handle`] values.
pub trait Dependencies<'id, O> {
    /// Returns the task ids of all prerequisite tasks.
    fn task_ids(&self) -> Vec<dag::TaskId<'id>>;

    /// Attempts to construct the input value for the dependant task.
    ///
    /// Returns `None` if any dependency output is not available.
    fn inputs(&self, execution: &Execution<'id>) -> Option<O>;
}

// no dependencies
impl<'id> Dependencies<'id, ()> for () {
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![]
    }

    fn inputs(&self, _execution: &Execution<'id>) -> Option<()> {
        Some(())
    }
}

// 1 dependency
impl<'id, T1> Dependencies<'id, (T1,)> for (dag::Handle<'id, T1>,)
where
    T1: Clone + 'static,
{
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![self.0.task_id()]
    }

    fn inputs(&self, execution: &Execution<'id>) -> Option<(T1,)> {
        let (i1,) = self;
        let i1 = execution.output_ref(*i1)?.clone();
        Some((i1,))
    }
}

// two dependencies
impl<'id, T1, T2> Dependencies<'id, (T1, T2)> for (dag::Handle<'id, T1>, dag::Handle<'id, T2>)
where
    T1: Clone + 'static,
    T2: Clone + 'static,
{
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![self.0.task_id(), self.1.task_id()]
    }

    fn inputs(&self, execution: &Execution<'id>) -> Option<(T1, T2)> {
        let (i1, i2) = self;
        let i1 = execution.output_ref(*i1)?.clone();
        let i2 = execution.output_ref(*i2)?.clone();
        Some((i1, i2))
    }
}

// three dependencies
impl<'id, T1, T2, T3> Dependencies<'id, (T1, T2, T3)>
    for (
        dag::Handle<'id, T1>,
        dag::Handle<'id, T2>,
        dag::Handle<'id, T3>,
    )
where
    T1: Clone + 'static,
    T2: Clone + 'static,
    T3: Clone + 'static,
{
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![self.0.task_id(), self.1.task_id(), self.2.task_id()]
    }

    fn inputs(&self, execution: &Execution<'id>) -> Option<(T1, T2, T3)> {
        let (i1, i2, i3) = self;
        let i1 = execution.output_ref(*i1)?.clone();
        let i2 = execution.output_ref(*i2)?.clone();
        let i3 = execution.output_ref(*i3)?.clone();
        Some((i1, i2, i3))
    }
}

// four dependencies
impl<'id, T1, T2, T3, T4> Dependencies<'id, (T1, T2, T3, T4)>
    for (
        dag::Handle<'id, T1>,
        dag::Handle<'id, T2>,
        dag::Handle<'id, T3>,
        dag::Handle<'id, T4>,
    )
where
    T1: Clone + 'static,
    T2: Clone + 'static,
    T3: Clone + 'static,
    T4: Clone + 'static,
{
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![
            self.0.task_id(),
            self.1.task_id(),
            self.2.task_id(),
            self.3.task_id(),
        ]
    }

    fn inputs(&self, execution: &Execution<'id>) -> Option<(T1, T2, T3, T4)> {
        let (i1, i2, i3, i4) = self;
        let i1 = execution.output_ref(*i1)?.clone();
        let i2 = execution.output_ref(*i2)?.clone();
        let i3 = execution.output_ref(*i3)?.clone();
        let i4 = execution.output_ref(*i4)?.clone();
        Some((i1, i2, i3, i4))
    }
}

// five dependencies
impl<'id, T1, T2, T3, T4, T5> Dependencies<'id, (T1, T2, T3, T4, T5)>
    for (
        dag::Handle<'id, T1>,
        dag::Handle<'id, T2>,
        dag::Handle<'id, T3>,
        dag::Handle<'id, T4>,
        dag::Handle<'id, T5>,
    )
where
    T1: Clone + 'static,
    T2: Clone + 'static,
    T3: Clone + 'static,
    T4: Clone + 'static,
    T5: Clone + 'static,
{
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![
            self.0.task_id(),
            self.1.task_id(),
            self.2.task_id(),
            self.3.task_id(),
            self.4.task_id(),
        ]
    }

    fn inputs(&self, execution: &Execution<'id>) -> Option<(T1, T2, T3, T4, T5)> {
        let (i1, i2, i3, i4, i5) = self;
        let i1 = execution.output_ref(*i1)?.clone();
        let i2 = execution.output_ref(*i2)?.clone();
        let i3 = execution.output_ref(*i3)?.clone();
        let i4 = execution.output_ref(*i4)?.clone();
        let i5 = execution.output_ref(*i5)?.clone();
        Some((i1, i2, i3, i4, i5))
    }
}

// six dependencies
impl<'id, T1, T2, T3, T4, T5, T6> Dependencies<'id, (T1, T2, T3, T4, T5, T6)>
    for (
        dag::Handle<'id, T1>,
        dag::Handle<'id, T2>,
        dag::Handle<'id, T3>,
        dag::Handle<'id, T4>,
        dag::Handle<'id, T5>,
        dag::Handle<'id, T6>,
    )
where
    T1: Clone + 'static,
    T2: Clone + 'static,
    T3: Clone + 'static,
    T4: Clone + 'static,
    T5: Clone + 'static,
    T6: Clone + 'static,
{
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![
            self.0.task_id(),
            self.1.task_id(),
            self.2.task_id(),
            self.3.task_id(),
            self.4.task_id(),
            self.5.task_id(),
        ]
    }

    fn inputs(&self, execution: &Execution<'id>) -> Option<(T1, T2, T3, T4, T5, T6)> {
        let (i1, i2, i3, i4, i5, i6) = self;
        let i1 = execution.output_ref(*i1)?.clone();
        let i2 = execution.output_ref(*i2)?.clone();
        let i3 = execution.output_ref(*i3)?.clone();
        let i4 = execution.output_ref(*i4)?.clone();
        let i5 = execution.output_ref(*i5)?.clone();
        let i6 = execution.output_ref(*i6)?.clone();
        Some((i1, i2, i3, i4, i5, i6))
    }
}

// seven dependencies
impl<'id, T1, T2, T3, T4, T5, T6, T7> Dependencies<'id, (T1, T2, T3, T4, T5, T6, T7)>
    for (
        dag::Handle<'id, T1>,
        dag::Handle<'id, T2>,
        dag::Handle<'id, T3>,
        dag::Handle<'id, T4>,
        dag::Handle<'id, T5>,
        dag::Handle<'id, T6>,
        dag::Handle<'id, T7>,
    )
where
    T1: Clone + 'static,
    T2: Clone + 'static,
    T3: Clone + 'static,
    T4: Clone + 'static,
    T5: Clone + 'static,
    T6: Clone + 'static,
    T7: Clone + 'static,
{
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![
            self.0.task_id(),
            self.1.task_id(),
            self.2.task_id(),
            self.3.task_id(),
            self.4.task_id(),
            self.5.task_id(),
            self.6.task_id(),
        ]
    }

    fn inputs(&self, execution: &Execution<'id>) -> Option<(T1, T2, T3, T4, T5, T6, T7)> {
        let (i1, i2, i3, i4, i5, i6, i7) = self;
        let i1 = execution.output_ref(*i1)?.clone();
        let i2 = execution.output_ref(*i2)?.clone();
        let i3 = execution.output_ref(*i3)?.clone();
        let i4 = execution.output_ref(*i4)?.clone();
        let i5 = execution.output_ref(*i5)?.clone();
        let i6 = execution.output_ref(*i6)?.clone();
        let i7 = execution.output_ref(*i7)?.clone();
        Some((i1, i2, i3, i4, i5, i6, i7))
    }
}

// eight dependencies
impl<'id, T1, T2, T3, T4, T5, T6, T7, T8> Dependencies<'id, (T1, T2, T3, T4, T5, T6, T7, T8)>
    for (
        dag::Handle<'id, T1>,
        dag::Handle<'id, T2>,
        dag::Handle<'id, T3>,
        dag::Handle<'id, T4>,
        dag::Handle<'id, T5>,
        dag::Handle<'id, T6>,
        dag::Handle<'id, T7>,
        dag::Handle<'id, T8>,
    )
where
    T1: Clone + 'static,
    T2: Clone + 'static,
    T3: Clone + 'static,
    T4: Clone + 'static,
    T5: Clone + 'static,
    T6: Clone + 'static,
    T7: Clone + 'static,
    T8: Clone + 'static,
{
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![
            self.0.task_id(),
            self.1.task_id(),
            self.2.task_id(),
            self.3.task_id(),
            self.4.task_id(),
            self.5.task_id(),
            self.6.task_id(),
            self.7.task_id(),
        ]
    }

    fn inputs(&self, execution: &Execution<'id>) -> Option<(T1, T2, T3, T4, T5, T6, T7, T8)> {
        let (i1, i2, i3, i4, i5, i6, i7, i8) = self;
        let i1 = execution.output_ref(*i1)?.clone();
        let i2 = execution.output_ref(*i2)?.clone();
        let i3 = execution.output_ref(*i3)?.clone();
        let i4 = execution.output_ref(*i4)?.clone();
        let i5 = execution.output_ref(*i5)?.clone();
        let i6 = execution.output_ref(*i6)?.clone();
        let i7 = execution.output_ref(*i7)?.clone();
        let i8 = execution.output_ref(*i8)?.clone();
        Some((i1, i2, i3, i4, i5, i6, i7, i8))
    }
}

impl<O> std::fmt::Debug for dyn Dependencies<'_, O> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dependencies").finish_non_exhaustive()
    }
}
