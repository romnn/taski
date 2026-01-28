use crate::{dag, execution::Execution};

pub trait Dependencies<'id, O> {
    fn task_ids(&self) -> Vec<dag::TaskId<'id>>;
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
    for (dag::Handle<'id, T1>, dag::Handle<'id, T2>, dag::Handle<'id, T3>)
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
impl<'id, T1, T2, T3, T4, T5, T6, T7, T8>
    Dependencies<'id, (T1, T2, T3, T4, T5, T6, T7, T8)>
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

    fn inputs(
        &self,
        execution: &Execution<'id>,
    ) -> Option<(T1, T2, T3, T4, T5, T6, T7, T8)> {
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

impl<'id, O> std::fmt::Debug for dyn Dependencies<'id, O> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.task_ids())
    }
}
