//! Execution tracing.
//!
//! A [`Trace`] records timing information for tasks executed by a [`crate::PolicyExecutor`].
//!
//! When the `render` feature is enabled, traces can also be rendered as SVG.

use crate::dag;
use std::time::{Duration, Instant};
use tracing::{Level, Span};

/// A traced task.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Human-readable label used for debugging and rendering.
    pub label: String,
    #[cfg(feature = "render")]
    /// Optional color used by the `render` feature.
    pub color: Option<crate::render::Rgba>,
    /// Start time.
    pub start: Instant,
    /// End time.
    pub end: Instant,
}

impl Task {
    /// Duration of the traced task.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.end.saturating_duration_since(self.start)
    }
}

/// An execution trace of tasks.
#[derive(Debug)]
pub struct Trace<T> {
    /// Wall-clock start time of the trace.
    pub start_time: Instant,
    /// Completed tasks and their traced timing information.
    pub tasks: Vec<(T, Task)>,
}

impl<T> Default for Trace<T>
where
    T: std::hash::Hash + std::fmt::Debug + std::cmp::Ord + Eq,
{
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

pub mod iter {
    //! Trace iterators.
    //!
    //! The main iterator is [`ConcurrentIter`], which yields the set of tasks that were running at
    //! each event boundary (task start/end).

    use std::collections::HashSet;
    use std::time::Instant;

    enum Event {
        Start(Instant),
        End(Instant),
    }

    impl Event {
        /// Returns the timestamp associated with this event.
        pub fn time(&self) -> &Instant {
            match self {
                Self::Start(t) | Self::End(t) => t,
            }
        }
    }

    /// An iterator over all combinations of concurrent tasks of a trace.
    #[allow(clippy::module_name_repetitions)]
    pub struct ConcurrentIter<'a, T> {
        events: std::vec::IntoIter<(&'a T, Event)>,
        running: HashSet<T>,
    }

    impl<'a, T> ConcurrentIter<'a, T> {
        /// Creates a new iterator over concurrency sets for the given trace tasks.
        pub fn new(tasks: &'a [(T, super::Task)]) -> Self {
            let mut events: Vec<_> = tasks
                .iter()
                .flat_map(|(k, t)| [(k, Event::Start(t.start)), (k, Event::End(t.end))])
                .collect();

            events.sort_by_key(|(_, event)| *event.time());
            Self {
                events: events.into_iter(),
                running: HashSet::new(),
            }
        }
    }

    impl<T> Iterator for ConcurrentIter<'_, T>
    where
        T: std::hash::Hash + Eq + Clone,
    {
        type Item = HashSet<T>;

        fn next(&mut self) -> Option<Self::Item> {
            let (k, event) = self.events.next()?;
            match event {
                Event::Start(_) => {
                    self.running.insert(k.clone());
                }
                Event::End(_) => {
                    self.running.remove(k);
                }
            }
            // TODO: use streaming iterators? but then move this into testing only to not add a
            // large dependency
            Some(self.running.clone())
        }
    }
}

impl<T> Trace<T> {
    #[must_use]
    #[inline]
    /// Creates an empty trace.
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            tasks: Vec::new(),
        }
    }

    /// Sort tasks in this trace based on their starting time
    pub fn sort_chronologically(&mut self) {
        // NOTE: this uses a stable sorting algorithm, i.e., it does not alter the order of
        // tasks that started at the same time.
        self.tasks.sort_by_key(|(_, Task { start, .. })| *start);
    }

    /// Iterator over all concurrent tasks
    #[must_use]
    pub fn iter_concurrent(&self) -> iter::ConcurrentIter<'_, T>
    where
        T: std::hash::Hash + Eq + Clone,
    {
        iter::ConcurrentIter::new(&self.tasks)
    }

    /// Maximum number of tasks that ran concurrently.
    #[must_use]
    pub fn max_concurrent(&self) -> usize
    where
        T: std::hash::Hash + Eq + Clone,
    {
        self.iter_concurrent()
            .map(|concurrent| concurrent.len())
            .max()
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug)]
pub struct TaskContext<'id, L> {
    pub task_id: dag::TaskId<'id>,
    pub task: crate::task::Ref<'id, L>,
    pub timeout: Option<Duration>,
}

impl<'id, L> TaskContext<'id, L> {
    #[must_use]
    pub fn task_name(&self) -> &str {
        self.task.name()
    }

    #[must_use]
    pub fn metadata(&self) -> &L {
        self.task.metadata()
    }

    #[must_use]
    pub fn dependencies(&self) -> &[dag::TaskId<'id>] {
        self.task.dependencies()
    }

    #[must_use]
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    #[cfg(feature = "render")]
    #[must_use]
    pub fn color(&self) -> &Option<crate::render::Rgba> {
        self.task.color()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TaskResultKind<'a> {
    Succeeded,
    TimedOut { timeout: Duration },
    Failed { err: &'a crate::task::Error },
}

pub trait MakeSpan<'id, L>: Clone + Send + Sync + 'id {
    fn make_span(&mut self, task: &TaskContext<'id, L>) -> Span;
}

pub trait OnTaskStart<'id, L>: Clone + Send + Sync + 'id {
    fn on_start(&mut self, task: &TaskContext<'id, L>, span: &Span);
}

pub trait OnTaskResult<'id, L>: Clone + Send + Sync + 'id {
    fn on_result(&mut self, task: &TaskContext<'id, L>, result: TaskResultKind<'_>, span: &Span);
}

pub trait TaskTrace<'id, L>: Clone + Send + Sync + 'id {
    fn make_span(&mut self, task: &TaskContext<'id, L>) -> Span;
    fn on_start(&mut self, task: &TaskContext<'id, L>, span: &Span);
    fn on_result(&mut self, task: &TaskContext<'id, L>, result: TaskResultKind<'_>, span: &Span);
}

#[derive(Debug, Clone, Default)]
pub struct NoopOnTaskStart;

impl<'id, L> OnTaskStart<'id, L> for NoopOnTaskStart {
    fn on_start(&mut self, _task: &TaskContext<'id, L>, _span: &Span) {}
}

#[derive(Debug, Clone, Default)]
pub struct NoopOnTaskResult;

impl<'id, L> OnTaskResult<'id, L> for NoopOnTaskResult {
    fn on_result(
        &mut self,
        _task: &TaskContext<'id, L>,
        _result: TaskResultKind<'_>,
        _span: &Span,
    ) {
    }
}

#[derive(Debug, Clone, Default)]
pub struct NoopMakeSpan;

impl<'id, L> MakeSpan<'id, L> for NoopMakeSpan {
    fn make_span(&mut self, _task: &TaskContext<'id, L>) -> Span {
        Span::none()
    }
}

#[derive(Debug, Clone)]
pub struct DefaultMakeSpan {
    level: Level,
}

impl Default for DefaultMakeSpan {
    fn default() -> Self {
        Self {
            level: Level::TRACE,
        }
    }
}

impl DefaultMakeSpan {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }
}

impl<'id, L> MakeSpan<'id, L> for DefaultMakeSpan
where
    L: 'id,
{
    fn make_span(&mut self, task: &TaskContext<'id, L>) -> Span {
        let task_index = task.task_id.idx().index();
        let task_name = task.task_name();
        let otel_name = format!("taski::{task_name}");

        macro_rules! make_span {
            ($level:expr) => {{
                tracing::span!(
                    $level,
                    "taski.task",
                    task_index,
                    task_name = %task_name,
                    {"perfetto.track_name"} = %task_name,
                    {"otel.name"} = %otel_name
                )
            }};
        }

        match self.level {
            Level::ERROR => make_span!(Level::ERROR),
            Level::WARN => make_span!(Level::WARN),
            Level::INFO => make_span!(Level::INFO),
            Level::DEBUG => make_span!(Level::DEBUG),
            Level::TRACE => make_span!(Level::TRACE),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskTraceLayer<M, S, R> {
    make_span: M,
    on_start: S,
    on_result: R,
}

pub type DefaultTaskTraceLayer = TaskTraceLayer<DefaultMakeSpan, NoopOnTaskStart, NoopOnTaskResult>;
pub type NoopTaskTraceLayer = TaskTraceLayer<NoopMakeSpan, NoopOnTaskStart, NoopOnTaskResult>;

impl Default for DefaultTaskTraceLayer {
    fn default() -> Self {
        Self {
            make_span: DefaultMakeSpan::default(),
            on_start: NoopOnTaskStart,
            on_result: NoopOnTaskResult,
        }
    }
}

impl Default for NoopTaskTraceLayer {
    fn default() -> Self {
        Self {
            make_span: NoopMakeSpan,
            on_start: NoopOnTaskStart,
            on_result: NoopOnTaskResult,
        }
    }
}

impl DefaultTaskTraceLayer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl NoopTaskTraceLayer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl<M, S, R> TaskTraceLayer<M, S, R> {
    #[must_use]
    pub fn make_span<M2>(self, make_span: M2) -> TaskTraceLayer<M2, S, R> {
        TaskTraceLayer {
            make_span,
            on_start: self.on_start,
            on_result: self.on_result,
        }
    }

    #[must_use]
    pub fn on_start<S2>(self, on_start: S2) -> TaskTraceLayer<M, S2, R> {
        TaskTraceLayer {
            make_span: self.make_span,
            on_start,
            on_result: self.on_result,
        }
    }

    #[must_use]
    pub fn on_result<R2>(self, on_result: R2) -> TaskTraceLayer<M, S, R2> {
        TaskTraceLayer {
            make_span: self.make_span,
            on_start: self.on_start,
            on_result,
        }
    }
}

impl<'id, L, M, S, R> TaskTrace<'id, L> for TaskTraceLayer<M, S, R>
where
    L: 'id,
    M: MakeSpan<'id, L>,
    S: OnTaskStart<'id, L>,
    R: OnTaskResult<'id, L>,
{
    fn make_span(&mut self, task: &TaskContext<'id, L>) -> Span {
        self.make_span.make_span(task)
    }

    fn on_start(&mut self, task: &TaskContext<'id, L>, span: &Span) {
        self.on_start.on_start(task, span);
    }

    fn on_result(&mut self, task: &TaskContext<'id, L>, result: TaskResultKind<'_>, span: &Span) {
        self.on_result.on_result(task, result, span);
    }
}
