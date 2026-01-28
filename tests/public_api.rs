use color_eyre::eyre;
use std::time::Duration;
use taski::dag;
use taski::{PolicyExecutor, Schedule, TaskResult};

async fn sleepy(v: u32, delay: Duration) -> TaskResult<u32> {
    tokio::time::sleep(delay).await;
    Ok(v)
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
async fn tuple_dependencies_work_up_to_eight_inputs() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let h1 = schedule.add_input(1_u32, ());
    let h2 = schedule.add_input(2_u32, ());
    let h3 = schedule.add_input(3_u32, ());
    let h4 = schedule.add_input(4_u32, ());
    let h5 = schedule.add_input(5_u32, ());
    let h6 = schedule.add_input(6_u32, ());
    let h7 = schedule.add_input(7_u32, ());
    let h8 = schedule.add_input(8_u32, ());

    let sum = schedule.add_closure(
        |a, b, c, d, e, f, g, h| async move { Ok::<_, Box<dyn std::error::Error + Send + Sync>>(a + b + c + d + e + f + g + h) },
        (h1, h2, h3, h4, h5, h6, h7, h8),
        (),
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

    async fn ok() -> TaskResult<u32> {
        Ok(10)
    }

    async fn fail() -> TaskResult<u32> {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "fail",
        )))
    }

    let ok_handle = schedule.add_closure(ok, (), ())?;
    let fail_handle = schedule.add_closure(fail, (), ())?;

    let mut executor = PolicyExecutor::fifo(schedule);
    let report = executor.run().await?;

    assert_eq!(report.total_tasks, 2);
    assert_eq!(report.succeeded_tasks, 1);
    assert_eq!(report.failed_tasks, 1);

    assert_eq!(executor.execution().output_ref(ok_handle).copied(), Some(10));
    assert!(executor.execution().output_ref(fail_handle).is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn priority_runs_higher_labels_first_with_single_concurrency() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    async fn ok_unit() -> TaskResult<()> {
        Ok(())
    }

    let _t1 = schedule.add_closure(|| ok_unit(), (), 1_u8)?;
    let _t2 = schedule.add_closure(|| ok_unit(), (), 3_u8)?;
    let _t3 = schedule.add_closure(|| ok_unit(), (), 2_u8)?;

    let mut executor = PolicyExecutor::priority(schedule).max_concurrent(Some(1));
    executor.run().await?;

    let labels_in_trace: Vec<_> = executor
        .trace()
        .tasks
        .iter()
        .map(|(task_id, _)| *executor.schedule().task_label(*task_id))
        .collect();

    assert_eq!(labels_in_trace, vec![3_u8, 2_u8, 1_u8]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn started_and_completed_timestamps_exist_for_traced_tasks() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let delay = Duration::from_millis(10);
    let _t1 = schedule.add_closure(move || sleepy(1, delay), (), ())?;
    let _t2 = schedule.add_closure(move || sleepy(2, delay), (), ())?;

    let mut executor = PolicyExecutor::fifo(schedule);
    executor.run().await?;

    for (task_id, _) in executor.trace().tasks.iter() {
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
    let _input = schedule.add_input(1_u32, ());

    let mut executor = PolicyExecutor::custom(schedule, NeverPolicy);

    let err = match executor.run().await {
        Ok(_report) => return Err(eyre::eyre!("expected stalled error")),
        Err(err) => err,
    };

    let taski::executor::RunError::Stalled {
        unfinished_tasks,
        pending_task_indices,
    } = err;

    assert_eq!(unfinished_tasks, 1);
    assert_eq!(pending_task_indices, vec![0]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_ref_is_none_before_run_and_some_after_run() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let one = schedule.add_input(1_i32, ());
    let two = schedule.add_input(2_i32, ());

    async fn add(lhs: i32, rhs: i32) -> TaskResult<i32> {
        Ok(lhs + rhs)
    }

    let three = schedule.add_closure(add, (one, two), ())?;

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
async fn dependant_fails_if_dependency_fails() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    async fn fail(_input: i32) -> TaskResult<i32> {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "fail",
        )))
    }

    async fn add_one(input: i32) -> TaskResult<i32> {
        Ok(input + 1)
    }

    let input = schedule.add_input(1_i32, ());
    let a = schedule.add_closure(fail, (input,), ())?;
    let b = schedule.add_closure(add_one, (a,), ())?;

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
async fn stalled_when_max_concurrent_is_zero() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let _input = schedule.add_input(1_u32, ());

    let mut executor = PolicyExecutor::fifo(schedule).max_concurrent(Some(0));

    let err = match executor.run().await {
        Ok(_report) => return Err(eyre::eyre!("expected stalled error")),
        Err(err) => err,
    };

    let taski::executor::RunError::Stalled {
        unfinished_tasks,
        pending_task_indices,
    } = err;

    assert_eq!(unfinished_tasks, 1);
    assert_eq!(pending_task_indices, vec![0]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fifo_respects_max_concurrency() -> eyre::Result<()> {
    taski::make_guard!(guard);
    let mut schedule = Schedule::new(guard);

    let delay = Duration::from_millis(30);

    let _t1 = schedule.add_closure(move || sleepy(1, delay), (), ())?;
    let _t2 = schedule.add_closure(move || sleepy(2, delay), (), ())?;
    let _t3 = schedule.add_closure(move || sleepy(3, delay), (), ())?;

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

    let one = schedule.add_input(1_i32, ());
    let two = schedule.add_input(2_i32, ());

    async fn add(lhs: i32, rhs: i32) -> TaskResult<i32> {
        Ok(lhs + rhs)
    }

    let _three = schedule.add_closure(add, (one, two), ())?;

    let executor = PolicyExecutor::fifo(schedule);

    let ready_count = executor
        .schedule()
        .ready(executor.execution())
        .filter(|task_id| executor.execution().state(*task_id).is_pending())
        .count();

    assert_eq!(ready_count, 2);

    Ok(())
}
