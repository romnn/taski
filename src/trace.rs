use std::time::{Duration, Instant};

/// A traced task.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub label: String,
    #[cfg(feature = "render")]
    pub color: Option<crate::render::Rgba>,
    pub start: Instant,
    pub end: Instant,
}

impl Task {
    /// Duration of the traced task.
    #[must_use]
    pub fn duraton(&self) -> Duration {
        self.end.saturating_duration_since(self.start)
    }
}

/// An execution trace of tasks.
#[derive(Debug)]
pub struct Trace<T> {
    pub start_time: Instant,
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
    use std::collections::HashSet;
    use std::time::Instant;

    enum Event {
        Start(Instant),
        End(Instant),
    }

    impl Event {
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

    impl<'a, T> Iterator for ConcurrentIter<'a, T>
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
    pub fn iter_concurrent(&self) -> iter::ConcurrentIter<T>
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
