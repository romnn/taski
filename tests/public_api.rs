use color_eyre::eyre;
use std::collections::VecDeque;
use std::time::Duration;
use taski::Dependencies;
use taski::dag;
use taski::{PolicyExecutor, Schedule, TaskResult};

async fn ok_unit() -> TaskResult<()> {
    Ok(())
}

async fn add_i32(lhs: i32, rhs: i32) -> TaskResult<i32> {
    Ok(lhs + rhs)
}

async fn fail_i32(_input: i32) -> TaskResult<i32> {
    Err(Box::new(std::io::Error::other("fail")))
}

async fn add_one_i32(input: i32) -> TaskResult<i32> {
    Ok(input + 1)
}

async fn add_one_u32(v: u32) -> TaskResult<u32> {
    Ok(v + 1)
}

async fn add_two_u32(v: u32) -> TaskResult<u32> {
    Ok(v + 2)
}

async fn sum_u32(a: u32, b: u32) -> TaskResult<u32> {
    Ok(a + b)
}

async fn ok_u32() -> TaskResult<u32> {
    Ok(10)
}

async fn fail_u32() -> TaskResult<u32> {
    Err(Box::new(std::io::Error::other("fail")))
}

async fn sleepy(v: u32, delay: Duration) -> TaskResult<u32> {
    tokio::time::sleep(delay).await;
    Ok(v)
}

#[derive(Default)]
struct NeverInputs;

impl<'id> Dependencies<'id, ()> for NeverInputs {
    fn task_ids(&self) -> Vec<dag::TaskId<'id>> {
        vec![]
    }

    fn inputs(&self, _execution: &taski::execution::Execution<'id>) -> Option<()> {
        None
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct NeverPolicy;

impl<'id, L> taski::Policy<'id, L> for NeverPolicy {
    fn reset(&mut self) {}

    fn on_task_ready(&mut self, _task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {}

    fn on_task_started(&mut self, _task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {}

    fn on_task_finished(
        &mut self,
        _task_id: dag::TaskId<'id>,
        _state: taski::task::State,
        _schedule: &Schedule<'id, L>,
    ) {
    }

    fn next_task(
        &mut self,
        _schedule: &Schedule<'id, L>,
        _execution: &taski::execution::Execution<'id>,
    ) -> Option<dag::TaskId<'id>> {
        None
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_schedule_returns_empty_report() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let schedule: Schedule<'_, ()> = Schedule::new(guard);

    let mut executor = PolicyExecutor::fifo(schedule);
    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 0);
    assert_eq!(report.succeeded_tasks, 0);
    assert_eq!(report.failed_tasks, 0);
    assert_eq!(executor.trace().tasks.len(), 0);
    assert_eq!(executor.trace().max_concurrent(), 0);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_fails_if_dependencies_inputs_are_unavailable() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let handle = schedule.add_closure(ok_unit, NeverInputs)?;

    let mut executor = PolicyExecutor::fifo(schedule);
    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 1);
    assert_eq!(report.succeeded_tasks, 0);
    assert_eq!(report.failed_tasks, 1);
    assert!(executor.execution().state(handle.task_id()).did_fail());

    // The task never ran (no future), so it should not appear in the trace.
    assert_eq!(executor.trace().tasks.len(), 0);

    Ok(())
}

#[derive(Debug, Clone)]
struct EventPolicyLog {
    reset_count: usize,
    ready: Vec<usize>,
    started: Vec<usize>,
    finished: Vec<(usize, taski::task::State)>,
}

#[derive(Debug, Clone)]
struct EventPolicy<'id> {
    log: std::sync::Arc<std::sync::Mutex<EventPolicyLog>>,
    ready_queue: VecDeque<dag::TaskId<'id>>,
}

impl EventPolicy<'_> {
    fn lock_log(&self) -> eyre::Result<std::sync::MutexGuard<'_, EventPolicyLog>> {
        self.log
            .lock()
            .map_err(|_| eyre::eyre!("failed to lock policy log"))
    }
}

impl<'id, L> taski::Policy<'id, L> for EventPolicy<'id> {
    fn reset(&mut self) {
        self.ready_queue.clear();
        if let Ok(mut guard) = self.lock_log() {
            guard.reset_count = guard.reset_count.saturating_add(1);
        }
    }

    fn on_task_ready(&mut self, task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {
        self.ready_queue.push_back(task_id);
        if let Ok(mut guard) = self.lock_log() {
            guard.ready.push(task_id.idx().index());
        }
    }

    fn on_task_started(&mut self, task_id: dag::TaskId<'id>, _schedule: &Schedule<'id, L>) {
        if let Ok(mut guard) = self.lock_log() {
            guard.started.push(task_id.idx().index());
        }
    }

    fn on_task_finished(
        &mut self,
        task_id: dag::TaskId<'id>,
        state: taski::task::State,
        _schedule: &Schedule<'id, L>,
    ) {
        if let Ok(mut guard) = self.lock_log() {
            guard.finished.push((task_id.idx().index(), state));
        }
    }

    fn next_task(
        &mut self,
        _schedule: &Schedule<'id, L>,
        execution: &taski::execution::Execution<'id>,
    ) -> Option<dag::TaskId<'id>> {
        while let Some(task_id) = self.ready_queue.pop_front() {
            if execution.state(task_id).is_pending() {
                return Some(task_id);
            }
        }
        None
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn policy_hooks_fire_once_per_task_in_successful_run() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let input = schedule.add_input(1_u32);

    let a = schedule.add_closure(add_one_u32, (input,))?;
    let b = schedule.add_closure(add_two_u32, (input,))?;
    let _c = schedule.add_closure(sum_u32, (a, b))?;

    let log = std::sync::Arc::new(std::sync::Mutex::new(EventPolicyLog {
        reset_count: 0,
        ready: Vec::new(),
        started: Vec::new(),
        finished: Vec::new(),
    }));

    let policy = EventPolicy {
        log: std::sync::Arc::clone(&log),
        ready_queue: VecDeque::new(),
    };

    let mut executor = PolicyExecutor::new(schedule, policy);
    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 4);
    assert_eq!(report.succeeded_tasks, 4);
    assert_eq!(report.failed_tasks, 0);

    let guard = log
        .lock()
        .map_err(|_| eyre::eyre!("failed to lock policy log"))?;
    assert_eq!(guard.reset_count, 1);

    let mut unique_ready = guard.ready.clone();
    unique_ready.sort_unstable();
    unique_ready.dedup();
    assert_eq!(unique_ready.len(), 4);

    let mut unique_started = guard.started.clone();
    unique_started.sort_unstable();
    unique_started.dedup();
    assert_eq!(unique_started.len(), 4);

    let mut unique_finished: Vec<_> = guard.finished.iter().map(|(idx, _)| *idx).collect();
    unique_finished.sort_unstable();
    unique_finished.dedup();
    assert_eq!(unique_finished.len(), 4);

    for (_, state) in &guard.finished {
        assert!(state.did_succeed());
    }

    Ok(())
}

#[cfg(feature = "render")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn render_smoke_tests_for_empty_and_non_empty() -> eyre::Result<()> {
    taski::make_guard!(guard_empty);
    let empty_schedule: Schedule<'_, ()> = Schedule::new(guard_empty);
    let empty_svg = empty_schedule.render();
    assert!(!empty_svg.is_empty());

    taski::make_guard!(guard_non_empty);
    let mut schedule = Schedule::new(guard_non_empty);
    let i1 = schedule.add_input(1_u32);

    let h = schedule.add_closure(add_one_u32, (i1,))?;

    let mut executor = PolicyExecutor::fifo(schedule);
    executor.run().await?;
    assert_eq!(executor.execution().output_ref(h).copied(), Some(2));

    let trace_svg = executor.trace().render()?;
    assert!(!trace_svg.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tuple_dependencies_work_up_to_eight_inputs() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let h1 = schedule.add_input(1_u32);
    let h2 = schedule.add_input(2_u32);
    let h3 = schedule.add_input(3_u32);
    let h4 = schedule.add_input(4_u32);
    let h5 = schedule.add_input(5_u32);
    let h6 = schedule.add_input(6_u32);
    let h7 = schedule.add_input(7_u32);
    let h8 = schedule.add_input(8_u32);

    let sum = schedule.add_closure(
        |a, b, c, d, e, f, g, h| async move {
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(a + b + c + d + e + f + g + h)
        },
        (h1, h2, h3, h4, h5, h6, h7, h8),
    )?;

    let mut executor = PolicyExecutor::fifo(schedule);
    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 9);
    assert_eq!(report.succeeded_tasks, 9);
    assert_eq!(report.failed_tasks, 0);
    assert_eq!(executor.execution().running_count(), 0);
    assert_eq!(executor.execution().unfinished_count(), 0);

    assert_eq!(executor.execution().output_ref(sum).copied(), Some(36));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_branch_can_succeed_even_if_other_branch_fails() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let ok_handle = schedule.add_closure(ok_u32, ())?;
    let fail_handle = schedule.add_closure(fail_u32, ())?;

    let mut executor = PolicyExecutor::fifo(schedule);
    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 2);
    assert_eq!(report.succeeded_tasks, 1);
    assert_eq!(report.failed_tasks, 1);

    assert_eq!(
        executor.execution().output_ref(ok_handle).copied(),
        Some(10)
    );
    assert!(executor.execution().output_ref(fail_handle).is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dependant_fails_if_dependency_fails() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let input = schedule.add_input(1_i32);
    let a = schedule.add_closure(fail_i32, (input,))?;
    let b = schedule.add_closure(add_one_i32, (a,))?;

    let mut executor = PolicyExecutor::fifo(schedule);
    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 3);
    assert_eq!(report.succeeded_tasks, 1);
    assert_eq!(report.failed_tasks, 2);

    assert!(executor.execution().state(a.task_id()).did_fail());
    assert!(executor.execution().state(b.task_id()).did_fail());
    assert!(executor.execution().output_ref(a).is_none());
    assert!(executor.execution().output_ref(b).is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dependant_fails_if_dependency_fails_and_failure_propagates() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let input = schedule.add_input(1_i32);
    let a = schedule.add_closure(fail_i32, (input,))?;
    let b = schedule.add_closure(add_one_i32, (a,))?;

    let mut executor = PolicyExecutor::fifo(schedule);
    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 3);
    assert_eq!(report.succeeded_tasks, 1);
    assert_eq!(report.failed_tasks, 2);

    assert!(executor.execution().state(a.task_id()).did_fail());
    assert!(executor.execution().state(b.task_id()).did_fail());
    assert!(executor.execution().output_ref(a).is_none());
    assert!(executor.execution().output_ref(b).is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn priority_runs_higher_labels_first_with_single_concurrency() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let _t1 = schedule.add_closure_with_metadata(ok_unit, (), 1_u8)?;
    let _t2 = schedule.add_closure_with_metadata(ok_unit, (), 3_u8)?;
    let _t3 = schedule.add_closure_with_metadata(ok_unit, (), 2_u8)?;

    let mut executor = PolicyExecutor::priority(schedule).max_concurrent(Some(1));
    executor.run().await?;

    let labels_in_trace: Vec<_> = executor
        .trace()
        .tasks
        .iter()
        .map(|(task_id, _)| *executor.schedule().task_metadata(*task_id))
        .collect();

    assert_eq!(labels_in_trace, vec![3_u8, 2_u8, 1_u8]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn started_and_completed_timestamps_exist_for_traced_tasks() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let delay = Duration::from_millis(10);
    let _t1 = schedule.add_closure(move || sleepy(1, delay), ())?;
    let _t2 = schedule.add_closure(move || sleepy(2, delay), ())?;

    let mut executor = PolicyExecutor::fifo(schedule);
    executor.run().await?;

    for (task_id, _) in &executor.trace().tasks {
        let started = executor
            .execution()
            .started_at(*task_id)
            .ok_or_else(|| eyre::eyre!("missing started_at"))?;
        let completed = executor
            .execution()
            .completed_at(*task_id)
            .ok_or_else(|| eyre::eyre!("missing completed_at"))?;

        assert!(completed >= started);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_when_policy_never_schedules_anything() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);
    let _input = schedule.add_input(1_u32);

    let mut executor = PolicyExecutor::new(schedule, NeverPolicy);

    let err = match executor.run().await {
        Ok(_report) => return Err(eyre::eyre!("expected stalled error")),
        Err(err) => err,
    };

    let taski::executor::RunError::Stalled {
        unfinished_tasks,
        pending_task_indices,
    } = err
    else {
        return Err(eyre::eyre!("expected stalled error"));
    };

    assert_eq!(unfinished_tasks, 1);
    assert_eq!(pending_task_indices, vec![0]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_ref_is_none_before_run_and_some_after_run() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let one = schedule.add_input(1_i32);
    let two = schedule.add_input(2_i32);

    let three = schedule.add_closure(add_i32, (one, two))?;

    let mut executor = PolicyExecutor::fifo(schedule);
    assert!(executor.execution().output_ref(three).is_none());

    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 3);
    assert_eq!(report.succeeded_tasks, 3);
    assert_eq!(report.failed_tasks, 0);

    let output = executor.execution().output_ref(three).copied();
    assert_eq!(output, Some(3));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_when_max_concurrent_is_zero() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let _input = schedule.add_input(1_u32);

    let mut executor = PolicyExecutor::fifo(schedule).max_concurrent(Some(0));

    let err = match executor.run().await {
        Ok(_report) => return Err(eyre::eyre!("expected stalled error")),
        Err(err) => err,
    };

    let taski::executor::RunError::Stalled {
        unfinished_tasks,
        pending_task_indices,
    } = err
    else {
        return Err(eyre::eyre!("expected stalled error"));
    };

    assert_eq!(unfinished_tasks, 1);
    assert_eq!(pending_task_indices, vec![0]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fifo_respects_max_concurrency() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let delay = Duration::from_millis(30);

    let _t1 = schedule.add_closure(move || sleepy(1, delay), ())?;
    let _t2 = schedule.add_closure(move || sleepy(2, delay), ())?;
    let _t3 = schedule.add_closure(move || sleepy(3, delay), ())?;

    let mut executor = PolicyExecutor::fifo(schedule).max_concurrent(Some(2));
    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 3);
    assert_eq!(report.succeeded_tasks, 3);
    assert_eq!(report.failed_tasks, 0);

    assert_eq!(executor.trace().max_concurrent(), 2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn schedule_ready_lists_tasks_without_dependencies() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let one = schedule.add_input(1_i32);
    let two = schedule.add_input(2_i32);

    let _three = schedule.add_closure(|lhs, rhs| async move { Ok(lhs + rhs) }, (one, two))?;

    let executor = PolicyExecutor::fifo(schedule);

    let ready_count = executor
        .schedule()
        .ready(executor.execution())
        .filter(|task_id| executor.execution().state(*task_id).is_pending())
        .count();

    assert_eq!(ready_count, 2);

    Ok(())
}
