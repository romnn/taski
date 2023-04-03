#![allow(warnings)]

use self::policy::{GreedyPolicy, Policy};
use async_trait::async_trait;
use futures::stream::StreamExt;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::pin::Pin;
use std::sync::RwLock;
use std::sync::{Arc, RwLockReadGuard};
use std::time::Instant;

#[derive(Debug)]
pub struct Product<H, T: IntoTuple>(pub(crate) H, pub(crate) T);

// Converts Product (and ()) into tuples.
pub trait IntoTuple {
    type Tuple: Tuple<Product = Self>;

    fn flatten(self) -> Self::Tuple;
}

// Typeclass that tuples can be converted into a Product (or unit ()).
pub trait Tuple: Sized {
    type Product: IntoTuple<Tuple = Self>;

    fn into_product(self) -> Self::Product;

    #[inline]
    fn combine<T>(self, other: T) -> CombinedTuples<Self, T>
    where
        Self: Sized,
        T: Tuple,
        Self::Product: Combine<T::Product>,
    {
        self.into_product().combine(other.into_product()).flatten()
    }
}

pub type CombinedTuples<T, U> =
    <<<T as Tuple>::Product as Combine<<U as Tuple>::Product>>::Output as IntoTuple>::Tuple;

// Combines Product together.
pub trait Combine<T: IntoTuple> {
    type Output: IntoTuple;

    fn combine(self, other: T) -> Self::Output;
}

impl<T: IntoTuple> Combine<T> for () {
    type Output = T;
    #[inline]
    fn combine(self, other: T) -> Self::Output {
        other
    }
}

impl<H, T: IntoTuple, U: IntoTuple> Combine<U> for Product<H, T>
where
    T: Combine<U>,
    Product<H, <T as Combine<U>>::Output>: IntoTuple,
{
    type Output = Product<H, <T as Combine<U>>::Output>;

    #[inline]
    fn combine(self, other: U) -> Self::Output {
        Product(self.0, self.1.combine(other))
    }
}

impl IntoTuple for () {
    type Tuple = ();
    #[inline]
    fn flatten(self) -> Self::Tuple {}
}

impl Tuple for () {
    type Product = ();

    #[inline]
    fn into_product(self) -> Self::Product {}
}

pub mod trace {
    use plotters::coord::Shift;
    use plotters::prelude::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Instant;
    use std::time::UNIX_EPOCH;
    use tokio::sync::Mutex;

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    #[inline]
    fn hue_to_rgb(hue: palette::RgbHue) -> RGBColor {
        use palette::IntoColor;
        let hsv = palette::Hsv::new(hue, 1.0, 1.0);
        let rgb: palette::rgb::Rgb = hsv.into_color();
        RGBColor(
            (rgb.red * 255.0) as u8,
            (rgb.green * 255.0) as u8,
            (rgb.blue * 255.0) as u8,
        )
    }

    #[derive(thiserror::Error, Debug)]
    pub enum RenderError {
        #[error("the trace is too large to be rendered")]
        TooLarge,
    }

    #[derive(Default, Debug, Clone, Hash, PartialEq, Eq)]
    pub struct Task {
        pub label: String,
        pub start: Option<Instant>,
        pub end: Option<Instant>,
    }

    #[derive(Debug)]
    pub struct Trace<T> {
        start_time: Instant,
        pub tasks: Mutex<HashMap<T, Task>>,
    }

    impl<T> Default for Trace<T>
    where
        T: std::hash::Hash + std::fmt::Display + std::fmt::Debug + std::cmp::Ord + Eq,
    {
        #[inline]
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T> Trace<T>
    where
        T: std::hash::Hash + std::fmt::Display + std::fmt::Debug + std::cmp::Ord + Eq,
    {
        #[must_use]
        #[inline]
        pub fn new() -> Self {
            Self {
                start_time: Instant::now(),
                tasks: Mutex::new(HashMap::new()),
            }
        }

        /// Render the trace as an SVG image.
        ///
        /// # Errors
        /// If the trace is too large to be rendered.
        #[allow(clippy::cast_possible_truncation)]
        pub async fn render(&self, path: impl AsRef<Path>) -> Result<(), RenderError> {
            #[derive(Default, Debug, Clone)]
            struct Bar<T> {
                begin: u128,
                length: u128,
                label: String,
                id: T,
                color: RGBColor,
            }

            const BAR_HEIGHT: i32 = 40;
            const TARGET_WIDTH: u32 = 2000;

            let tasks = self.tasks.lock().await;

            let mut bars: Vec<_> = tasks
                .iter()
                .filter_map(|(k, t)| match (t.start, t.end) {
                    (Some(s), Some(e)) => {
                        let begin: u128 = s.duration_since(self.start_time).as_millis();
                        // .try_into()
                        // .unwrap();
                        let end: u128 = e.duration_since(self.start_time).as_millis();
                        // .try_into()
                        // .unwrap();
                        Some(Bar {
                            begin,
                            length: end - begin,
                            label: t.label.clone(),
                            color: RGBColor(0, 0, 0),
                            id: k,
                        })
                    }
                    _ => None,
                })
                .collect();

            // assign colors to the tasks
            let mut rng = ChaCha8Rng::seed_from_u64(0);
            let colors = std::iter::repeat_with(|| {
                let hue = palette::RgbHue::from_degrees(rng.gen_range(0.0..360.0));
                hue_to_rgb(hue)
            });
            bars.sort_by(|a, b| {
                if a.begin == b.begin {
                    a.id.cmp(b.id)
                } else {
                    a.begin.cmp(&b.begin)
                }
            });

            for (mut bar, color) in bars.iter_mut().zip(colors) {
                bar.color = color;
            }
            // dbg!(&bars);

            // compute the earliest start and latest end time for normalization
            let earliest = bars.iter().map(|b| b.begin).min();
            let latest = bars.iter().map(|b| b.begin + b.length).max();

            let height = u32::try_from(bars.len()).map_err(|_| RenderError::TooLarge)?
                * u32::try_from(BAR_HEIGHT).map_err(|_| RenderError::TooLarge)?
                + 5;
            let bar_width = f64::from(TARGET_WIDTH - 200) / latest.unwrap_or(0) as f64;

            let size = (TARGET_WIDTH, height);
            let drawing_area = SVGBackend::new(path.as_ref(), size).into_drawing_area();
            let font = ("monospace", BAR_HEIGHT - 10).into_font();
            let text_style = TextStyle::from(font).color(&BLACK);
            for (i, bar) in bars.iter().enumerate() {
                let i = i32::try_from(i).unwrap();
                let rect = [
                    ((bar_width * bar.begin as f64) as i32, BAR_HEIGHT * i),
                    (
                        (bar_width * (bar.begin + bar.length) as f64) as i32 + 2,
                        BAR_HEIGHT * (i + 1),
                    ),
                ];
                drawing_area
                    .draw(&Rectangle::new(
                        rect,
                        ShapeStyle {
                            color: bar.color.to_rgba(),
                            filled: true,
                            stroke_width: 0,
                        },
                    ))
                    .unwrap();
                drawing_area
                    .draw_text(
                        &bar.label,
                        &text_style,
                        (
                            (bar_width * bar.begin as f64) as i32 + 1,
                            BAR_HEIGHT * i + 5,
                        ),
                    )
                    .unwrap();
            }
            Ok(())
        }
    }
}

#[derive(Debug)]
enum State<I, O> {
    /// Task is pending and waiting to be run
    Pending(Box<dyn Task<I, O> + Send + Sync + 'static>),
    /// Task is running
    Running,
    /// Task succeeded with the desired output
    Succeeded(O),
    /// Task failed with an error
    Failed(Arc<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(Debug, Clone)]
pub enum CompletionResult {
    /// Task is pending
    Pending,
    /// Task is running
    Running,
    /// Task succeeded
    Succeeded,
    /// Task failed with an error
    Failed(Arc<dyn std::error::Error + Send + Sync + 'static>),
}

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum TaskError {
    #[error("task dependency failed")]
    FailedDependency,
}

/// A task node in the task graph.
///
/// The task node tracks the state of the tasks lifecycle and is assigned a unique index.
/// It is not possible to directly construct a task node to enforce correctness.
/// References to `TaskNode` can be used as dependencies.
///
/// `TaskNodes` are only generic (static) over the inputs and outputs, since that is of
/// importance for using a task node as a dependency for another task
pub struct TaskNode<I, O, L> {
    task_name: String,
    short_task_name: String,
    label: L,
    state: RwLock<State<I, O>>,
    dependencies: Box<dyn Dependencies<I> + Send + Sync>,
    index: usize,
}

impl<I, O, L> Hash for TaskNode<I, O, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // hash the index
        self.index.hash(state);
    }
}

impl<I, O, L> std::fmt::Display for TaskNode<I, O, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_task_name)
    }
}

impl<I, O, L> std::fmt::Debug for TaskNode<I, O, L>
where
    I: std::fmt::Debug,
    O: std::fmt::Debug,
    L: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskNode")
            .field("id", &self.index)
            .field("task", &self.task_name)
            .field("label", &self.label)
            .field("state", &self.state.read().unwrap())
            .field("dependencies", &self.dependencies)
            .finish()
    }
}

pub mod policy {
    use crate::Executor;

    use super::{Schedule, TaskRef};
    // use async_trait::async_trait;

    pub trait Policy {
        fn arbitrate<P>(&self, schedule: &Executor<P>) -> Option<TaskRef>
        where
            P: Policy;
        // fn arbitrate(
        //     &self,
        //     schedule: &Schedule,
        // ) -> Option<TaskRef>;
        // where
        // I: Send + Sync + Eq + Hash + std::fmt::Debug + 'static,
        // C: Send + Sync + 'static,
        // O: Send + Sync + 'static,
        // E: Send + Sync + std::fmt::Debug + 'static;
    }

    pub struct GreedyPolicy {
        max_tasks: Option<usize>,
    }

    impl GreedyPolicy {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn max_tasks(max_tasks: Option<usize>) -> Self {
            Self { max_tasks }
        }
    }

    impl Default for GreedyPolicy {
        fn default() -> Self {
            Self {
                max_tasks: Some(num_cpus::get()),
            }
        }
    }

    impl Policy for GreedyPolicy {
        fn arbitrate<P>(&self, schedule: &Executor<P>) -> Option<TaskRef>
        where
            P: Policy,
        {
            // count some labels
            dbg!(schedule.running().iter().count());
            let num_downloads = schedule
                .ready()
                // .running()
                // .iter()
                // .filter(|t: &&TaskRef| {
                .filter(|t: &TaskRef| {
                    let label = t.label();
                    // dbg!(label.downcast_ref::<TaskLabel>());
                    // let label = t.as_any().downcast_ref();
                    //     match t.label() {
                    // }
                    false
                })
                .count();
            dbg!(num_downloads);
            schedule.ready().next()
        }
    }
}

pub mod dfs {
    #[allow(missing_debug_implementations)]
    #[derive(Clone)]
    pub struct Dfs<'a, N, F> {
        stack: Vec<(usize, &'a N)>,
        graph: &'a super::DAG<N>,
        max_depth: Option<usize>,
        filter: F,
    }

    impl<'a, N, F> Dfs<'a, N, F>
    where
        N: std::hash::Hash + Eq,
        F: Fn(&&N) -> bool,
    {
        #[inline]
        pub fn new(
            graph: &'a super::DAG<N>,
            root: &'a N,
            max_depth: impl Into<Option<usize>>,
            filter: F,
        ) -> Self {
            let mut stack = vec![];
            if let Some(children) = graph.get(root) {
                stack.extend(children.iter().map(|child| (1, child)));
            }
            Self {
                stack,
                graph,
                max_depth: max_depth.into(),
                filter,
            }
        }

        // this gives lifetime errors
        // fn children(&self, node: &'a N) -> Option<impl Iterator<Item = &'a N> + '_> {
        //     self.graph
        //         .get(&node)
        //         .map(|children| children.iter().filter(&self.filter))
        // }
    }

    impl<'a, N, F> Iterator for Dfs<'a, N, F>
    where
        N: std::hash::Hash + Eq,
        F: Fn(&&N) -> bool,
        // F: Fn(&'a N) -> bool,
    {
        type Item = (usize, &'a N);

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            match self.stack.pop() {
                Some((depth, node)) => {
                    if let Some(max_depth) = self.max_depth {
                        if depth >= max_depth {
                            return Some((depth, node));
                        }
                    }
                    if let Some(children) = self.graph.get(&node) {
                        self.stack.extend(
                            children
                                .iter()
                                .filter(&self.filter)
                                .map(|child| (depth + 1, child)),
                        );
                    };
                    Some((depth, node))
                }
                None => None,
            }
        }
    }
}

pub type DAG<N> = HashMap<N, HashSet<N>>;

pub trait Traversal<N> {
    fn traverse<'a, D, F>(&'a self, root: &'a N, depth: D, filter: F) -> dfs::Dfs<'a, N, F>
    where
        F: Fn(&&N) -> bool,
        N: std::hash::Hash + Eq,
        D: Into<Option<usize>>;
}

impl<N> Traversal<N> for DAG<N> {
    fn traverse<'a, D, F>(&'a self, root: &'a N, depth: D, filter: F) -> dfs::Dfs<'a, N, F>
    where
        F: Fn(&&N) -> bool,
        N: std::hash::Hash + Eq,
        D: Into<Option<usize>>,
    {
        dfs::Dfs::new(self, root, depth.into(), filter)
    }
}

pub type TaskRef = Arc<dyn Schedulable>;

/// A task schedule based on a DAG of task nodes.
#[derive(Default)]
pub struct Schedule {
    dependencies: DAG<TaskRef>,
    dependants: DAG<TaskRef>,
}

/// An executor for a task schedule
#[derive(Debug)]
pub struct Executor<P> {
    schedule: Schedule,
    policy: P,
    trace: Arc<trace::Trace<usize>>,
    running: Arc<RwLock<HashSet<TaskRef>>>,
    ready: Vec<TaskRef>,
}

impl std::fmt::Debug for Schedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Schedule")
            .field("dependencies", &self.dependencies)
            .field("dependants", &self.dependants)
            .finish()
    }
}

impl Schedule {
    /// Add a new task to the graph.
    ///
    /// Dependencies for the task must be references to tasks that have
    /// already been added to the task graph (Arc<TaskNode>)
    pub fn add_node<I, O, L, T, D>(&mut self, task: T, deps: D, label: L) -> Arc<TaskNode<I, O, L>>
    where
        T: Task<I, O> + std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
        D: Dependencies<I> + Send + Sync + 'static,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Sync + 'static,
    {
        let node = Arc::new(TaskNode {
            task_name: format!("{task}"),
            short_task_name: format!("{task:?}"),
            label,
            // label: task.label(),
            state: RwLock::new(State::Pending(Box::new(task))),
            dependencies: Box::new(deps),
            index: self.dependencies.len(),
        });

        // check for circles here
        let mut seen: HashSet<TaskRef> = HashSet::new();
        let mut stack: Vec<TaskRef> = vec![node.clone()];

        while let Some(node) = stack.pop() {
            if !seen.insert(node.clone()) {
                continue;
            }
            let dependencies = self
                .dependencies
                .entry(node.clone())
                .or_insert(HashSet::new());

            for dep in node.dependencies() {
                let dependants = self.dependants.entry(dep.clone()).or_insert(HashSet::new());
                dependants.insert(node.clone());
                dependencies.insert(dep.clone());
                stack.push(dep);
            }
        }

        node
    }

    pub fn fail_dependants<'a>(&'a mut self, root: &'a TaskRef, dependencies: bool) {
        let mut queue = vec![root];
        let mut processed = HashSet::new();
        processed.insert(root);

        while let Some(task) = queue.pop() {
            dbg!(&queue);
            dbg!(&processed);

            // fail all dependants
            let mut new_processed = vec![];
            let dependants = self.dependants.traverse(task, None, |dep: &&TaskRef| {
                matches!(dep.state(), CompletionResult::Pending) && !processed.contains(dep)
            });
            for (_, dependant) in dependants {
                // todo: link to failed dependency
                // if processed.insert(dependant) {
                new_processed.push(dependant);
                dependant.fail(Arc::new(TaskError::FailedDependency));
                // processed.insert(dependant);
                queue.push(dependant);
                // }
            }
            // fail all dependencies of task
            if dependencies {
                // fail all dependencies
                let dependencies = self.dependencies.traverse(task, None, |dep: &&TaskRef| {
                    matches!(dep.state(), CompletionResult::Pending) && !processed.contains(task)
                });
                for (_, dependency) in dependencies {
                    // fail dependency
                    // if processed.insert(dependency) {
                    dependency.fail(Arc::new(TaskError::FailedDependency));
                    // processed.insert(dependency);
                    queue.push(dependency);
                    new_processed.push(dependency);
                    // }
                }
            }

            processed.extend(new_processed);
        }
    }

    /// Iterator over the immediate dependencies of a task
    pub fn dependencies<'a>(
        &'a self,
        task: &'a TaskRef,
    ) -> dfs::Dfs<'a, TaskRef, impl Fn(&&TaskRef) -> bool> {
        self.dependencies.traverse(task, Some(1), |_| true)
    }

    /// Iterator over the immediate dependants of a task
    pub fn dependants<'a>(
        &'a self,
        task: &'a TaskRef,
    ) -> dfs::Dfs<'a, TaskRef, impl Fn(&&TaskRef) -> bool> {
        self.dependants.traverse(task, Some(1), |_| true)
    }

    /// Iterator over all recursive dependencies of a task
    pub fn rec_dependencies<'a>(
        &'a self,
        task: &'a TaskRef,
    ) -> dfs::Dfs<'a, TaskRef, impl Fn(&&TaskRef) -> bool> {
        self.dependencies.traverse(task, None, |_| true)
    }

    /// Iterator over all recursive dependants of a task
    pub fn rec_dependants<'a>(
        &'a self,
        task: &'a TaskRef,
    ) -> dfs::Dfs<'a, TaskRef, impl Fn(&&TaskRef) -> bool> {
        self.dependants.traverse(task, None, |_| true)
    }

    /// Render the task graph as an svg image.
    ///
    /// # Errors
    /// If writing to the specified output path fails.
    pub fn render_to(&self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        use layout::backends::svg::SVGWriter;
        use layout::core::{self, base::Orientation, color::Color, style};
        use layout::std_shapes::shapes;
        use layout::topo::layout::VisualGraph;
        use std::io::{BufWriter, Write};

        fn node(node: &Arc<dyn Schedulable>) -> shapes::Element {
            let node_style = style::StyleAttr {
                line_color: Color::new(0x0000_00FF),
                line_width: 2,
                fill_color: Some(Color::new(0xB4B3_B2FF)),
                rounded: 0,
                font_size: 15,
            };
            let size = core::geometry::Point { x: 100.0, y: 100.0 };
            shapes::Element::create(
                shapes::ShapeKind::Circle(format!("{node}")),
                node_style,
                Orientation::TopToBottom,
                size,
            )
        }

        let mut graph = VisualGraph::new(Orientation::TopToBottom);

        let mut handles: HashMap<Arc<dyn Schedulable>, layout::adt::dag::NodeHandle> =
            HashMap::new();

        for (task, deps) in &self.dependencies {
            let dest_handle = *handles
                .entry(task.clone())
                .or_insert_with(|| graph.add_node(node(task)));
            for dep in deps {
                let src_handle = *handles
                    .entry(dep.clone())
                    .or_insert_with(|| graph.add_node(node(dep)));
                let arrow = shapes::Arrow {
                    start: shapes::LineEndKind::None,
                    end: shapes::LineEndKind::Arrow,
                    line_style: style::LineStyleKind::Normal,
                    text: String::new(),
                    look: style::StyleAttr {
                        line_color: Color::new(0x0000_00FF),
                        line_width: 2,
                        fill_color: Some(Color::new(0xB4B3_B2FF)),
                        rounded: 0,
                        font_size: 15,
                    },
                    src_port: None,
                    dst_port: None,
                };
                graph.add_edge(arrow, src_handle, dest_handle);
            }
        }

        // https://docs.rs/layout-rs/latest/src/layout/backends/svg.rs.html#200
        let mut backend = SVGWriter::new();
        let debug_mode = false;
        let disable_opt = false;
        let disable_layout = false;
        graph.do_it(debug_mode, disable_opt, disable_layout, &mut backend);
        let content = backend.finalize();

        let file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(path.as_ref())?;
        let mut writer = BufWriter::new(file);
        writer.write_all(content.as_bytes())?;
        Ok(())
    }
}

impl<P> Executor<P>
where
    P: Policy,
{
    /// Iterator over all ready tasks
    pub fn ready<'a>(&'a self) -> impl Iterator<Item = TaskRef> + '_ {
        self.ready.iter().cloned()
    }

    /// Iterator over all running tasks
    pub fn running<'a>(&'a self) -> RwLockReadGuard<HashSet<TaskRef>> {
        self.running.read().unwrap()
    }

    /// Render the execution trace as an svg image.
    ///
    /// # Errors
    /// If writing to the specified output path fails.
    pub async fn render_trace(&self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        self.trace.render(path.as_ref()).await;
        Ok(())
    }

    /// Runs the tasks in the graph
    pub async fn run(&mut self) {
        use futures::stream::FuturesUnordered;
        use std::future::Future;

        type TaskFut = dyn Future<Output = TaskRef>;
        let mut tasks: FuturesUnordered<Pin<Box<TaskFut>>> = FuturesUnordered::new();

        // let mut ready: Vec<TaskRef> = self
        self.ready = self
            .schedule
            .dependencies
            .keys()
            .filter(|t| t.ready())
            .cloned()
            .collect();
        dbg!(&self.ready);

        loop {
            // check if we are done
            if tasks.is_empty() && self.ready.is_empty() {
                println!("we are done");
                break;
            }

            // start running ready tasks
            // while let Some(p) = self.policy.arbitrate(&self.schedule) {
            while let Some(p) = self.policy.arbitrate(&self) {
                self.ready.retain(|r| r != &p);
                // for p in ready.drain(0..) {
                let trace = self.trace.clone();
                let running = self.running.clone();
                tasks.push(Box::pin(async move {
                    println!("running {:?}", &p);

                    running.write().unwrap().insert(p.clone());
                    trace.tasks.lock().await.insert(
                        p.index(),
                        trace::Task {
                            label: p.short_name(),
                            start: Some(Instant::now()),
                            end: None,
                        },
                    );
                    p.run().await;
                    if let Some(mut task) = trace.tasks.lock().await.get_mut(&p.index()) {
                        task.end = Some(Instant::now());
                    }
                    p
                }));
            }

            // wait for a task to complete
            if let Some(completed) = tasks.next().await {
                // todo: check for output or error
                println!("task {} completed: {:?}", &completed, completed.state());
                self.running.write().unwrap().remove(&completed);
                match completed.state() {
                    CompletionResult::Pending | CompletionResult::Running => {
                        unreachable!("completed task state is invalid");
                    }
                    CompletionResult::Failed(err) => {
                        // fail fast
                        self.schedule.fail_dependants(&completed, true);
                    }
                    CompletionResult::Succeeded => {}
                }
                // assert!(matches!(State::Pending(_), &completed));

                if let Some(dependants) = &self.schedule.dependants.get(&completed) {
                    println!("dependants: {:?}", &dependants);
                    // extend the ready queue
                    self.ready.extend(dependants.iter().filter_map(|d| {
                        if d.ready() {
                            Some(d.clone())
                        } else {
                            None
                        }
                    }));
                }
            }
        }
    }
}

/// Trait representing a schedulable task node.
///
/// TaskNodes implement this trait.
/// We cannot just use the TaskNode by itself, because we need to combine
/// task nodes with different generic parameters.
#[async_trait]
pub trait Schedulable {
    /// Indicates if the schedulable task is ready for execution.
    ///
    /// A task is ready if all its dependencies succeeded.
    fn ready(&self) -> bool {
        self.dependencies().iter().all(|d| d.succeeded())
    }

    /// Indicates if the schedulable task has succeeded.
    ///
    /// A task is succeeded if its output is available.
    fn succeeded(&self) -> bool;

    /// Fails the schedulable task.
    fn fail(&self, err: Arc<dyn std::error::Error + Send + Sync + 'static>);

    /// The result state of the task after completion.
    fn state(&self) -> CompletionResult;

    /// Unique index of the task node in the DAG graph
    fn index(&self) -> usize;

    /// Run the schedulable task using the dependencies' outputs as input.
    ///
    /// Running the task does not require a mutable borrow, since we
    /// only swap out the internal state which is protected using interior mutability.
    /// A schedulable task can only run exactly once, since the inner task
    /// is consumed and only the output or error is kept.
    ///
    /// Note: The user must ensure that this is only called when
    /// all dependencies have completed. (todo: change that)
    async fn run(&self);

    /// Returns the name of the schedulable task
    // fn as_any(&self) -> &dyn Any;

    /// Returns the name of the schedulable task
    fn name(&self) -> String;

    /// Returns the short name of the schedulable task
    fn short_name(&self) -> String;

    /// Returns the labels of this task
    fn label(&self) -> &dyn Any;
    // fn labels(&self) -> L;

    /// Returns the dependencies of the schedulable task
    fn dependencies(&self) -> Vec<Arc<dyn Schedulable>>;
}

impl std::fmt::Debug for dyn Schedulable + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::fmt::Debug for dyn Schedulable + Send + Sync + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::fmt::Display for dyn Schedulable + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

impl std::fmt::Display for dyn Schedulable + Send + Sync + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

impl Hash for dyn Schedulable + '_ {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index().hash(state);
    }
}

impl Hash for dyn Schedulable + Send + Sync + '_ {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index().hash(state);
    }
}

impl PartialEq for dyn Schedulable + '_ {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(&self.index(), &other.index())
    }
}

impl PartialEq for dyn Schedulable + Send + Sync + '_ {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(&self.index(), &other.index())
    }
}

impl Eq for dyn Schedulable + '_ {}

impl Eq for dyn Schedulable + Send + Sync + '_ {}

#[async_trait]
impl<I, O, L> Schedulable for TaskNode<I, O, L>
where
    I: std::fmt::Debug + Send + Sync + 'static,
    O: std::fmt::Debug + Send + Sync + 'static,
    L: std::fmt::Debug + Sync + 'static, //  + Sync + 'static,
{
    fn succeeded(&self) -> bool {
        matches!(*self.state.read().unwrap(), State::Succeeded(_))
    }

    fn fail(&self, err: Arc<dyn std::error::Error + Send + Sync + 'static>) {
        *self.state.write().unwrap() = State::Failed(err);
    }

    fn state(&self) -> CompletionResult {
        match &*self.state.read().unwrap() {
            State::Pending(_) => CompletionResult::Pending,
            State::Running => CompletionResult::Running,
            State::Succeeded(_) => CompletionResult::Succeeded,
            State::Failed(err) => CompletionResult::Failed(err.clone()),
        }
    }

    fn index(&self) -> usize {
        self.index
    }

    async fn run(&self) {
        // get the inputs from the dependencies
        let inputs = self.dependencies.inputs().unwrap();
        println!("running task {self:?}({inputs:?})");
        let state = {
            let state = &mut *self.state.write().unwrap();
            if let State::Pending(_) = state {
                // returns owned previous value (pending task)
                std::mem::replace(state, State::Running)
            } else {
                // already done
                return;
            }
        };
        if let State::Pending(task) = state {
            // before consuming, we should extract the labels
            //
            {
                // let pls = task.as_ref();
                let pls = task.as_any();

                // let downcasted = pls.downcast_ref::<String>();
                // let downcasted = pls.downcast_ref::<Box<dyn Label<TaskLabel>>>();
                // let downcasted = pls.downcast_ref::<&dyn Label<TaskLabel>>();
                // let downcasted = pls.downcast_ref::<&dyn Label<TaskLabel>>();
                // eprintln!("downcasted: {:?}", &downcasted.map(|l| l.label()));
                // eprintln!("downcasted: {:?}", &downcasted);
                // .downcast_ref::<Box<dyn Label<TaskLabel> + '_>>()
                // if let Some(t) = pls // .downcast_ref::<Box<dyn Label<TaskLabel> + '_>>()
                //
                // {
                //     eprintln!("downcasted: {:?}", &t);
                //     // dbg!(t);
                //     // dbg!(t.label());
                // }
                // drop(pls);
            }
            // this will consume the task
            let result = task.run(inputs).await;

            // check if task was already marked as failed
            let state = &mut *self.state.write().unwrap();
            if !matches!(state, State::Running) {
                return;
            }
            *state = match result {
                Ok(output) => State::Succeeded(output),
                Err(err) => State::Failed(err.into()),
            };
        }
    }

    /// Returns the name of the schedulable task
    // fn as_any(&self) -> &dyn Any {
    //     self.task.as_any()
    // }

    fn label(&self) -> &dyn Any {
        &self.label
    }

    fn name(&self) -> String {
        format!("{self:?}")
    }

    fn short_name(&self) -> String {
        format!("{self}")
    }

    fn dependencies(&self) -> Vec<Arc<dyn Schedulable>> {
        self.dependencies.to_vec()
    }
}

pub trait Dependency<O>: Schedulable {
    fn output(&self) -> Option<O>;
}

impl<I, O, L> Dependency<O> for TaskNode<I, O, L>
where
    I: std::fmt::Debug + Send + Sync + 'static,
    O: std::fmt::Debug + Clone + Send + Sync + 'static,
    L: std::fmt::Debug + Sync + 'static,
{
    fn output(&self) -> Option<O> {
        match &*self.state.read().unwrap() {
            State::Succeeded(output) => Some(output.clone()),
            _ => None,
        }
    }
}

pub trait Dependencies<O> {
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable>>;
    fn inputs(&self) -> Option<O>;
}

// no dependencies
impl Dependencies<()> for () {
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable>> {
        vec![]
    }

    fn inputs(&self) -> Option<()> {
        Some(())
    }
}

// 1 dependency
impl<D1, T1> Dependencies<(T1,)> for (Arc<D1>,)
where
    D1: Dependency<T1> + 'static,
    T1: Clone,
{
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable>> {
        vec![self.0.clone() as Arc<dyn Schedulable>]
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
impl<D1, D2, T1, T2> Dependencies<(T1, T2)> for (Arc<D1>, Arc<D2>)
where
    D1: Dependency<T1> + 'static,
    D2: Dependency<T2> + 'static,
    T1: Clone,
    T2: Clone,
{
    fn to_vec(&self) -> Vec<Arc<dyn Schedulable>> {
        vec![
            self.0.clone() as Arc<dyn Schedulable>,
            self.1.clone() as Arc<dyn Schedulable>,
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

impl<O> std::fmt::Debug for dyn Dependencies<O> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}",
            self.to_vec().iter().map(|d| d.index()).collect::<Vec<_>>()
        )
    }
}

impl<O> std::fmt::Debug for dyn Dependencies<O> + Send + Sync + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}",
            self.to_vec().iter().map(|d| d.index()).collect::<Vec<_>>()
        )
    }
}

pub type TaskResult<O> = Result<O, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[async_trait]
pub trait Task<I, O>: std::fmt::Debug {
    /// Running a task consumes it, which does guarantee that tasks
    /// may only run exactly once.
    async fn run(self: Box<Self>, input: I) -> TaskResult<O>;

    // fn as_any<'a>(&'a self) -> Box<dyn Any + '_> {
    // fn as_any(&self) -> Box<dyn Any + '_> {
    fn as_any(&self) -> &dyn Any;
    // fn as_any(&self) -> &dyn Any where Self: Sized + 'static {
    //     self
    //     // Box::new(self)
    //     // Box::new(self)
    // }
    // fn as_any(&self) -> &(dyn Any + '_);

    /// The name of the task
    /// todo: add debug bound here
    fn name(&self) -> String {
        format!("{self:?}")
    }

    // fn label(&self) -> Option<L> {
    //     None
    // }
}

/// Task that is labeled.
pub trait Label<L> {
    fn label(&self) -> L;
}

/// A simple terminal input for a task
#[derive(Clone, Debug)]
pub struct TaskInput<O>(O);

impl<O> From<O> for TaskInput<O> {
    #[inline]
    fn from(value: O) -> Self {
        Self(value)
    }
}

/// Implements the Task trait for task inputs.
///
/// All that this implementation does is return a shared reference to
/// the task input.
#[async_trait]
impl<O> Task<(), O> for TaskInput<O>
where
    O: std::fmt::Debug + Send + 'static,
{
    async fn run(self: Box<Self>, _input: ()) -> TaskResult<O> {
        Ok(self.0)
    }

    // fn as_any(self: &Box<Self>) -> Box<dyn Any> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    //     Box::new(self)
    // }
}

fn summarize(s: impl AsRef<str>, max_length: usize) -> String {
    let s = s.as_ref();
    if s.len() > max_length {
        format!(
            "{}...{}",
            &s[..(max_length / 2)],
            &s[s.len() - (max_length / 2)..s.len()]
        )
    } else {
        s.to_string()
    }
}

impl<O> std::fmt::Display for TaskInput<O>
where
    O: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let short_value = summarize(format!("{}", &self.0), 20);
        f.debug_tuple("TaskInput").field(&short_value).finish()
    }
}

macro_rules! task {
        ($name:ident: $( $type:ident ),*) => {
            #[allow(non_snake_case)]
            #[async_trait]
            pub trait $name<$( $type ),*, O>: std::fmt::Debug {
                async fn run(self: Box<Self>, $($type: $type),*) -> TaskResult<O>;

                // fn as_any(&self) -> &(dyn Any + '_) {
                // fn as_any(&self) -> &dyn Any;
                // {
                //     self
                // }
            }

            #[allow(non_snake_case)]
            #[async_trait]
            impl<T, $( $type ),*, O> Task<($( $type ),*,), O> for T
            where
                T: $name<$( $type ),*, O> + Send + 'static,
                $($type: std::fmt::Debug + Send + 'static),*
            {
                async fn run(self: Box<Self>, input: ($( $type ),*,)) -> TaskResult<O> {
                    // destructure to tuple and call
                    let ($( $type ),*,) = input;
                    $name::run(self, $( $type ),*).await
                }

                // fn as_any(&self) -> &(dyn Any + '_) {
                fn as_any(&self) -> &dyn Any {
                    self
                }
            }
        }
    }

macro_rules! product {
        ($H:expr) => { Product($H, ()) };
        ($H:expr, $($T:expr),*) => { Product($H, product!($($T),*)) };
    }

macro_rules! Product {
        ($H:ty) => { Product<$H, ()> };
        ($H:ty, $($T:ty),*) => { Product<$H, Product!($($T),*)> };
    }

macro_rules! product_pat {
        ($H:pat) => { Product($H, ()) };
        ($H:pat, $($T:pat),*) => { Product($H, product_pat!($($T),*)) };
    }

macro_rules! generics {
    ($type:ident) => {
        impl<$type> IntoTuple for Product!($type) {
            type Tuple = ($type,);

            #[inline]
            fn flatten(self) -> Self::Tuple {
                (self.0,)
            }
        }

        impl<$type> Tuple for ($type,) {
            type Product = Product!($type);
            #[inline]
            fn into_product(self) -> Self::Product {
                product!(self.0)
            }
        }
    };


    ($type1:ident, $( $type:ident ),*) => {
        generics!($( $type ),*);

        impl<$type1, $( $type ),*> IntoTuple for Product!($type1, $($type),*) {
            type Tuple = ($type1, $( $type ),*);

            #[inline]
            fn flatten(self) -> Self::Tuple {
                #[allow(non_snake_case)]
                let product_pat!($type1, $( $type ),*) = self;
                ($type1, $( $type ),*)
            }
        }

        impl<$type1, $( $type ),*> Tuple for ($type1, $($type),*) {
            type Product = Product!($type1, $( $type ),*);

            #[inline]
            fn into_product(self) -> Self::Product {
                #[allow(non_snake_case)]
                let ($type1, $( $type ),*) = self;
                product!($type1, $( $type ),*)
            }
        }
    };
}

// task!(Task0: ());
task!(Task1: T1);
task!(Task2: T1, T2);
task!(Task3: T1, T2, T3);

generics! {
    T1,
    T2,
    T3
    // T4,
    // T5,
    // T6,
    // T7,
    // T8,
    // T9,
    // T10,
    // T11,
    // T12,
    // T13,
    // T14,
    // T15,
    // T16
}

// pub fn arbitrate(schedule: &mut Schedule) -> Option<TaskRef> {
//     // count some labels
//     let num_downloads = schedule
//         .running()
//         .iter()
//         .filter(|t: &&TaskRef| {
//             // let label = t.as_any().downcast_ref();
//             //     match t.label() {
//             // }
//             false
//         })
//         .count();
//     dbg!(num_downloads);
//     schedule.ready().next()
// }

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use core::{
        any::{Any, TypeId},
        mem,
    };
    use downcast_trait::DowncastTrait;
    use std::path::PathBuf;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_basic_scheduler() -> Result<()> {
        #[derive(Clone, Debug)]
        struct Identity {}

        #[derive(Debug)]
        enum TaskLabel {
            Input,
            Identity,
            Combine,
        }

        impl DowncastTrait for Identity {
            downcast_trait::downcast_trait_impl_convert_to!(dyn Label<TaskLabel>);
        }

        impl Label<TaskLabel> for Identity {
            fn label(&self) -> TaskLabel {
                TaskLabel::Identity
            }
        }

        impl std::fmt::Display for Identity {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", &self)
            }
        }

        #[async_trait]
        impl Task1<String, String> for Identity {
            async fn run(self: Box<Self>, input: String) -> TaskResult<String> {
                println!("identity with input: {input:?}");
                Ok(input)
            }

            // fn as_any(&self) -> &dyn Any {
            //     self
            // }
        }

        // #[async_trait]
        // impl Task<(String,), String> for Identity {
        //     async fn run(self: Box<Self>, input: (String,)) -> String {
        //         println!("identity with input: {:?}", input);
        //         input.0
        //     }
        // }

        #[derive(Clone, Debug)]
        struct Combine {}

        impl Label<TaskLabel> for Combine {
            fn label(&self) -> TaskLabel {
                TaskLabel::Combine
            }
        }

        impl std::fmt::Display for Combine {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", &self)
            }
        }

        #[async_trait]
        impl Task2<String, String, String> for Combine {
            async fn run(self: Box<Self>, a: String, b: String) -> TaskResult<String> {
                println!("combine with input: {:?}", (&a, &b));
                Ok(format!("{} {}", &a, &b))
            }

            // fn as_any(&self) -> &dyn Any {
            //     self
            // }
        }

        #[derive(Clone, Debug, Default)]
        struct CustomPolicy {}

        impl Policy for CustomPolicy {
            fn arbitrate<P>(&self, schedule: &Executor<P>) -> Option<TaskRef>
            where
                P: Policy,
            {
                // count some labels
                dbg!(schedule.running().iter().count());
                let num_downloads = schedule
                    .ready()
                    // .running()
                    // .iter()
                    // .filter(|t: &&TaskRef| {
                    .filter(|t: &TaskRef| {
                        let label = t.label();
                        dbg!(label.downcast_ref::<TaskLabel>());
                        // let label = t.as_any().downcast_ref();
                        //     match t.label() {
                        // }
                        false
                    })
                    .count();
                dbg!(num_downloads);
                schedule.ready().next()
            }
        }

        let combine = Combine {};
        let identity = Identity {};

        let mut graph = Schedule::default();

        let input_node =
            graph.add_node(TaskInput::from("George".to_string()), (), TaskLabel::Input);

        let base_node = graph.add_node(identity.clone(), (input_node,), TaskLabel::Identity);
        let parent1_node =
            graph.add_node(identity.clone(), (base_node.clone(),), TaskLabel::Identity);
        let parent2_node =
            graph.add_node(identity.clone(), (base_node.clone(),), TaskLabel::Identity);
        let result_node = graph.add_node(
            combine.clone(),
            (parent1_node, parent2_node),
            TaskLabel::Combine,
        );
        dbg!(&graph);

        graph.render_to(
            PathBuf::from(file!())
                .parent()
                .unwrap()
                .join("../graphs/basic.svg"),
        )?;

        let mut executor = Executor {
            schedule: graph,
            running: Arc::new(RwLock::new(HashSet::new())),
            trace: Arc::new(trace::Trace::new()),
            policy: CustomPolicy::default(),
            ready: Vec::new(),
        };
        // run all tasks
        executor.run().await;

        // debug the graph now
        dbg!(&executor.schedule);

        // assert the output value of the scheduler is correct
        assert_eq!(result_node.output(), Some("George George".to_string()));

        assert!(false);

        Ok(())
    }
}
