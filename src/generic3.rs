use async_trait::async_trait;
use futures::stream::StreamExt;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::RwLock;

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

#[derive(Debug)]
pub enum State<I, O> {
    /// Task is pending and waiting to be run
    Pending(Box<dyn Task<I, O> + Send + Sync + 'static>),
    /// Task is running
    Running,
    /// Task succeeded with the desired output
    Succeeded(O),
    /// Task failed with an error
    Failed(Box<dyn std::error::Error + Send + Sync + 'static>),
}

/// A task node in the task graph.
///
/// The task node tracks the state of the tasks lifecycle and is assigned a unique index.
/// It is not possible to directly construct a task node to enforce correctness.
/// References to `TaskNode` can be used as dependencies.
///
/// `TaskNodes` are only generic (static) over the inputs and outputs, since that is of
/// importance for using a task node as a dependency for another task
pub struct TaskNode<I, O> {
    name: String,
    state: RwLock<State<I, O>>,
    dependencies: Box<dyn Dependencies<I> + Send + Sync>,
    index: usize,
}

impl<I, O> Hash for TaskNode<I, O> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // hash the index
        self.index.hash(state);
    }
}

impl<I, O> std::fmt::Debug for TaskNode<I, O>
where
    I: std::fmt::Debug,
    O: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskNode")
            .field("id", &self.index)
            .field("task", &self.name)
            .field("state", &self.state.read().unwrap())
            .finish()
    }
}

/// A DAG graph of task nodes.
#[derive(Default, Debug)]
pub struct TaskGraph {
    dependencies: HashMap<Arc<dyn Schedulable>, HashSet<Arc<dyn Schedulable>>>,
    dependants: HashMap<Arc<dyn Schedulable>, HashSet<Arc<dyn Schedulable>>>,
}

impl TaskGraph {
    /// Add a new task to the graph.
    ///
    /// Dependencies for the task must be references to tasks that have already been added
    /// to the task graph (Arc<TaskNode>)
    pub fn add_node<I, O, T>(
        &mut self,
        task: T,
        dependencies: Box<dyn Dependencies<I> + Send + Sync>,
    ) -> Arc<TaskNode<I, O>>
    where
        T: Task<I, O> + Send + Sync + 'static,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
    {
        let node = Arc::new(TaskNode {
            name: task.name(),
            state: RwLock::new(State::Pending(Box::new(task))),
            dependencies,
            index: self.dependencies.len(),
        });

        // check for circles here
        let mut seen: HashSet<Arc<dyn Schedulable>> = HashSet::new();
        let mut stack: Vec<Arc<dyn Schedulable>> = vec![node.clone()];

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

    /// Runs the tasks in the graph, consuming it (for now).
    pub async fn run(&mut self) {
        use futures::stream::FuturesUnordered;
        use std::future::Future;

        type TaskFut = dyn Future<Output = Arc<dyn Schedulable>>;
        let mut tasks: FuturesUnordered<Pin<Box<TaskFut>>> = FuturesUnordered::new();

        let mut ready: Vec<Arc<dyn Schedulable>> = self
            .dependencies
            .keys()
            .filter(|t| t.ready())
            .cloned()
            .collect();
        dbg!(&ready);

        loop {
            // check if we are done
            if tasks.is_empty() && ready.is_empty() {
                println!("we are done");
                break;
            }

            // start running ready tasks
            for p in ready.drain(0..) {
                tasks.push(Box::pin(async move {
                    println!("running {:?}", &p);
                    p.run().await;
                    p
                }));
            }

            // wait for a task to complete
            if let Some(completed) = tasks.next().await {
                // todo: check for output or error
                println!("task completed {:?}", &completed);

                if let Some(dependants) = &self.dependants.get(&completed) {
                    println!("dependants: {:?}", &dependants);
                    ready.extend(dependants.iter().filter_map(|d| {
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
    fn ready(&self) -> bool;

    /// Indicates if the schedulable task has succeeded.
    ///
    /// A task is succeeded if its output is available.
    fn succeeded(&self) -> bool;

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
    fn name(&self) -> String;

    /// Returns the dependencies of the schedulable task
    fn dependencies(&self) -> Vec<Arc<dyn Schedulable>>;
}

impl std::fmt::Debug for dyn Schedulable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Hash for dyn Schedulable {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index().hash(state);
    }
}

impl PartialEq for dyn Schedulable {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(&self.index(), &other.index())
    }
}

impl Eq for dyn Schedulable {}

#[async_trait]
impl<I, O> Schedulable for TaskNode<I, O>
where
    I: std::fmt::Debug + Send + Sync,
    O: std::fmt::Debug + Send + Sync,
{
    fn ready(&self) -> bool {
        self.dependencies().iter().all(|d| d.succeeded())
    }

    fn succeeded(&self) -> bool {
        matches!(*self.state.read().unwrap(), State::Succeeded(_))
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
                std::mem::replace(state, State::Running)
            } else {
                // already done
                return;
            }
        };
        if let State::Pending(task) = state {
            // this will consume the task
            let result = task.run(inputs).await;
            let state = &mut *self.state.write().unwrap();
            *state = match result {
                Ok(output) => State::Succeeded(output),
                Err(err) => State::Failed(err),
            };
        }
    }

    fn name(&self) -> String {
        format!("{self:?}")
    }

    fn dependencies(&self) -> Vec<Arc<dyn Schedulable>> {
        self.dependencies.to_vec()
    }
}

pub trait Dependency<O>: Schedulable {
    fn output(&self) -> Option<O>;
}

impl<I, O> Dependency<O> for TaskNode<I, O>
where
    I: std::fmt::Debug + Send + Sync + 'static,
    O: std::fmt::Debug + Clone + Send + Sync + 'static,
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

type TaskResult<O> = Result<O, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[async_trait]
pub trait Task<I, O>: std::fmt::Debug {
    /// Running a task consumes it, which does guarantee that tasks
    /// may only run exactly once.
    async fn run(self: Box<Self>, input: I) -> TaskResult<O>;

    /// The name of the task
    /// todo: add debug bound here
    fn name(&self) -> String {
        format!("{self:?}")
    }
}

/// A simple terminal input for a task
#[derive(Clone)]
pub struct TaskInput<O> {
    value: O,
}

impl<O> From<O> for TaskInput<O> {
    #[inline]
    fn from(value: O) -> Self {
        Self { value }
    }
}

/// Implements the Task trait for task inputs.
///
/// All that this implementation does is return a shared reference to
/// the task input.
#[async_trait]
impl<O> Task<(), O> for TaskInput<O>
where
    O: std::fmt::Debug + Send,
{
    async fn run(self: Box<Self>, _input: ()) -> TaskResult<O> {
        println!("task input {:?}", &self.value);
        Ok(self.value)
    }
}

impl<O> std::fmt::Debug for TaskInput<O>
where
    O: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TaskInput({:?})", &self.value)
    }
}

macro_rules! task {
        ($name:ident: $( $type:ident ),*) => {
            #[allow(non_snake_case)]
            #[async_trait]
            pub trait $name<$( $type ),*, O>: std::fmt::Debug {
                async fn run(self: Box<Self>, $($type: $type),*) -> TaskResult<O>;

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
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};

    fn render_graph(task_graph: &TaskGraph, path: impl AsRef<Path>) -> Result<()> {
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
                shapes::ShapeKind::Circle(node.name()),
                node_style,
                Orientation::TopToBottom,
                size,
            )
        }

        let mut graph = VisualGraph::new(Orientation::TopToBottom);

        let mut handles: HashMap<Arc<dyn Schedulable>, layout::adt::dag::NodeHandle> =
            HashMap::new();

        for (task, deps) in &task_graph.dependencies {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_basic_scheduler() -> Result<()> {
        #[derive(Clone, Debug)]
        struct Identity {}

        #[async_trait]
        impl Task1<String, String> for Identity {
            async fn run(self: Box<Self>, input: String) -> TaskResult<String> {
                println!("identity with input: {input:?}");
                Ok(input)
            }
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

        #[async_trait]
        impl Task2<String, String, String> for Combine {
            async fn run(self: Box<Self>, a: String, b: String) -> TaskResult<String> {
                println!("combine with input: {:?}", (&a, &b));
                Ok(format!("{} {}", &a, &b))
            }
        }

        let combine = Combine {};
        let identity = Identity {};

        let mut graph = TaskGraph::default();

        let input_node = graph.add_node(TaskInput::from("George".to_string()), Box::new(()));

        let base_node = graph.add_node(identity.clone(), Box::new((input_node,)));
        let parent1_node = graph.add_node(identity.clone(), Box::new((base_node.clone(),)));
        let parent2_node = graph.add_node(identity.clone(), Box::new((base_node.clone(),)));
        let result_node = graph.add_node(combine.clone(), Box::new((parent1_node, parent2_node)));
        dbg!(&graph);

        render_graph(
            &graph,
            PathBuf::from(file!())
                .parent()
                .unwrap()
                .join("../graphs/basic.svg"),
        )?;

        // run all tasks
        graph.run().await;
        // debug the graph now
        dbg!(&graph);

        // assert the output value of the scheduler is correct
        assert_eq!(result_node.output(), Some("George George".to_string()));

        Ok(())
    }
}
