mod warp {
    use async_trait::async_trait;
    use by_address::ByThinAddress;
    use futures::future::BoxFuture;
    use futures::stream::{FuturesUnordered, StreamExt};
    use std::collections::{HashMap, HashSet};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    #[derive(Debug)]
    pub struct Product<H, T: IntoTuple>(pub(crate) H, pub(crate) T);

    // Converts Product (and ()) into tuples.
    // pub trait IntoTuple: Sized {
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

    // call dependencies with arguments expanded
    pub struct TaskNode<I, O, T>
    where
        T: Task<I, O> + std::fmt::Debug,
    {
        task: T,
        dependencies: Box<dyn Dependencies<I> + Send + Sync>,
        output: Option<O>,
        phantom: std::marker::PhantomData<I>,
    }

    impl<I, O, T> std::fmt::Debug for TaskNode<I, O, T>
    where
        T: Task<I, O> + std::fmt::Debug,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(&self.task, f)
        }
    }

    // call dependencies with arguments expanded
    // pub struct NewTaskNode<I, O, T>
    // where
    //     T: Task<I, O> + std::fmt::Debug,
    // {
    //     task: T,
    //     // dependencies: Box<dyn Dependencies<I> + Send + Sync>,
    //     // output: Option<O>,
    //     // phantom: std::marker::PhantomData<I>,
    // }

    #[derive(Default, Debug)]
    pub struct TaskGraph {
        // dependencies: HashMap<
        //     ByThinAddress<Arc<dyn Schedulable>>,
        //     HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
        // >,
        // dependants: HashMap<
        //     ByThinAddress<Arc<dyn Schedulable>>,
        //     HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
        // >,
    }

    #[derive(Default, Debug)]
    pub struct TaskRef<I, O> {
        input: std::marker::PhantomData<I>,
        output: std::marker::PhantomData<O>,
    }

    impl TaskGraph {
        // pub fn add_node(&mut self, node: TaskNode) -> TaskRef {
        // pub fn add_node(&mut self, node: impl Schedulable) -> String {
        //     "test".into()
        // }
        pub fn add_node<I, O, T>(&mut self, node: TaskNode<I, O, T>) -> TaskRef<I, O>
        where
            T: Task<I, O> + std::fmt::Debug,
        {
            TaskRef {
                input: std::marker::PhantomData,
                output: std::marker::PhantomData,
            }
            // "test".into()
        }
    }

    #[derive(Default, Debug)]
    pub struct Scheduler {
        dependencies: HashMap<
            ByThinAddress<Arc<dyn Schedulable>>,
            HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
        >,
        dependants: HashMap<
            ByThinAddress<Arc<dyn Schedulable>>,
            HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
        >,
    }

    impl Scheduler {
        // pub fn add_task(&mut self, task: impl TaskNode<I, O>) {
        pub fn add_task<I, O, T>(&mut self, task: Arc<TaskNode<I, O, T>>)
        where
            T: Task<I, O> + std::fmt::Debug + Send + Sync + 'static,
            I: std::fmt::Debug + Send + Sync + 'static,
            O: std::fmt::Debug + Send + Sync + 'static,
        {
            // sanity check that partial eq works
            assert_eq!(ByThinAddress(task.clone()), ByThinAddress(task.clone()));

            // sanity check that hash works
            use std::hash::Hash;
            let hasher = std::collections::hash_map::DefaultHasher::new();
            assert_eq!(
                ByThinAddress(task.clone()).hash(&mut hasher.clone()),
                ByThinAddress(task.clone()).hash(&mut hasher.clone())
            );

            // check for circles here
            let mut seen: HashSet<ByThinAddress<Arc<dyn Schedulable>>> = HashSet::new();
            let mut stack: Vec<ByThinAddress<Arc<dyn Schedulable>>> = vec![ByThinAddress(task)];
            while let Some(task) = stack.pop() {
                if !seen.insert(task.clone()) {
                    continue;
                }
                let mut dependencies = self
                    .dependencies
                    .entry(task.clone())
                    .or_insert(HashSet::new());

                for dep in task.dependencies() {
                    let mut dependants = self
                        .dependants
                        .entry(ByThinAddress(dep.clone()))
                        .or_insert(HashSet::new());
                    dependants.insert(task.clone());
                    dependencies.insert(ByThinAddress(dep.clone()));
                    stack.push(ByThinAddress(dep));
                }
            }
        }
    }

    #[async_trait]
    pub trait Schedulable {
        /// is ready for execution
        fn ready(&self) -> bool;

        /// task has completed
        fn completed(&self) -> bool;

        /// run the task
        async fn run(&mut self);

        /// redirect format
        fn debug(&self) -> &dyn std::fmt::Debug;

        /// get the dependencies
        fn dependencies(&self) -> Vec<Arc<dyn Schedulable>>;
    }

    impl std::fmt::Debug for dyn Schedulable {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(self.debug(), f)
        }
    }

    #[async_trait]
    impl<O> Schedulable for TaskInput<O>
    where
        O: Sync + Send + std::fmt::Debug,
    {
        fn ready(&self) -> bool {
            true
        }

        fn completed(&self) -> bool {
            true
        }

        async fn run(&mut self) {
            // do nothing
        }

        fn debug(&self) -> &dyn std::fmt::Debug {
            self
        }

        fn dependencies(&self) -> Vec<Arc<dyn Schedulable>> {
            vec![]
        }
    }

    impl<O> std::fmt::Debug for TaskInput<O>
    where
        O: Sync + std::fmt::Debug,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TaskInput({:?})", &self.value)
        }
    }

    #[async_trait]
    impl<I, O, T> Schedulable for TaskNode<I, O, T>
    where
        T: Task<I, O> + std::fmt::Debug + Send + Sync,
        I: std::fmt::Debug + Send + Sync,
        O: std::fmt::Debug + Send + Sync,
    {
        /// a task is ready if its dependencies have completed
        fn ready(&self) -> bool {
            self.dependencies().iter().all(|d| d.completed())
        }

        /// a task is completed if its output is available
        fn completed(&self) -> bool {
            self.output.is_some()
        }

        async fn run(&mut self) {
            let inputs = self.dependencies.inputs().unwrap();
            println!("running task {:?}({:?})", self, inputs);
            // get the inputs from the dependencies
            self.output = Some(self.task.run(inputs).await);
        }

        fn debug(&self) -> &dyn std::fmt::Debug {
            self
        }

        fn dependencies(&self) -> Vec<Arc<dyn Schedulable>> {
            self.dependencies.to_vec()
        }
    }

    pub trait Dependency<O>: Schedulable {
        fn output(&self) -> Option<&O>;
    }

    impl<O> Dependency<O> for TaskInput<O>
    where
        O: Sync + Send + std::fmt::Debug + 'static,
    {
        fn output(&self) -> Option<&O> {
            Some(&self.value)
        }
    }

    impl<I, O, T> Dependency<O> for TaskNode<I, O, T>
    where
        T: Task<I, O> + std::fmt::Debug + Send + Sync + 'static,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
    {
        fn output(&self) -> Option<&O> {
            self.output.as_ref()
        }
    }

    pub trait Dependencies<O> {
        fn to_vec(&self) -> Vec<Arc<dyn Schedulable>>;
        fn inputs(&self) -> Option<O>;
    }

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
                (Some(i1),) => Some((i1.clone(),)),
                _ => None,
            }
        }
    }

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
                (Some(i1), Some(i2)) => Some((i1.clone(), i2.clone())),
                _ => None,
            }
        }
    }

    #[async_trait]
    pub trait Task<I, O> {
        async fn run(&mut self, input: I) -> O;
    }

    pub struct TaskInput<O> {
        value: O,
    }

    impl<O> From<O> for TaskInput<O> {
        fn from(value: O) -> Self {
            Self { value }
        }
    }

    #[async_trait]
    pub trait TaskNew {
        type Input;
        type Output;

        async fn run(&mut self, input: Self::Input) -> Self::Output;
    }

    #[async_trait]
    pub trait ManualTask1<T1, O> {
        async fn run(&mut self, t1: T1) -> O;
    }

    #[async_trait]
    pub trait ManualTask2<T1, T2, O> {
        async fn run(&mut self, t1: T1, t2: T2) -> O;
    }

    macro_rules! task {
        ($name:ident: $( $type:ident ),*) => {
            #[async_trait]
            pub trait $name<$( $type ),*, O> {
                async fn run(&mut self, $($type: $type),*) -> O;
            }
            #[async_trait]
            impl<T, $( $type ),*, O> Task<($( $type ),*,), O> for T
            where
                T: $name<$( $type ),*, O> + Send + 'static,
                $($type: Send + 'static),*
            {
                async fn run(&mut self, input: ($( $type ),*,)) -> O {
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

    pub struct InFlightTaskFut<O>
    where
        Self: Future,
    {
        fut: Pin<Box<dyn Future<Output = O> + Send + Sync>>,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use anyhow::Result;
        use async_trait::async_trait;
        use std::path::{Path, PathBuf};
        use tokio::sync::Mutex;

        fn render_graph(scheduler: &Scheduler, path: impl AsRef<Path>) -> Result<()> {
            use layout::backends::svg::SVGWriter;
            use layout::core::{self, base::Orientation, color::Color, style};
            use layout::gv::GraphBuilder;
            use layout::std_shapes::shapes;
            use layout::topo::layout::VisualGraph;
            use std::io::{BufWriter, Write};

            let mut graph = VisualGraph::new(Orientation::TopToBottom);

            fn node(node: &ByThinAddress<Arc<dyn Schedulable>>) -> shapes::Element {
                let node_style = style::StyleAttr {
                    line_color: Color::new(0x000000FF),
                    line_width: 2,
                    fill_color: Some(Color::new(0xB4B3B2FF)),
                    rounded: 0,
                    font_size: 15,
                };
                let size = core::geometry::Point { x: 100.0, y: 100.0 };
                shapes::Element::create(
                    shapes::ShapeKind::Circle(format!("{:?}", node.0.debug())),
                    node_style.clone(),
                    Orientation::TopToBottom,
                    size,
                )
            }

            let mut handles: HashMap<
                ByThinAddress<Arc<dyn Schedulable>>,
                layout::adt::dag::NodeHandle,
            > = HashMap::new();
            for (task, deps) in &scheduler.dependencies {
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
                        text: "".to_string(),
                        look: style::StyleAttr {
                            line_color: Color::new(0x000000FF),
                            line_width: 2,
                            fill_color: Some(Color::new(0xB4B3B2FF)),
                            rounded: 0,
                            font_size: 15,
                        },
                        src_port: None,
                        dst_port: None,
                    };
                    graph.add_edge(arrow, src_handle, dest_handle);
                }
            }

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
            writer.write_all(content.as_bytes());
            Ok(())
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn test_basic_scheduler() -> Result<()> {
            #[derive(Clone, Debug)]
            struct Identity {}

            #[async_trait]
            impl Task<(String,), String> for Identity {
                async fn run(&mut self, input: (String,)) -> String {
                    println!("identity with input: {:?}", input);
                    input.0
                }
            }

            #[derive(Clone, Debug)]
            struct Combine {}

            #[async_trait]
            impl Task2<String, String, String> for Combine {
                async fn run(&mut self, a: String, b: String) -> String {
                    println!("combine with input: {:?}", (&a, &b));
                    // let (a, b) = input;
                    format!("{} {}", &a, &b)
                }
            }

            let mut combine = Combine {};
            let mut identity = Identity {};

            let mut graph = TaskGraph::default();
            // let mut base = graph.add_task(identity);

            // make task nodes
            // let mut base = Arc::new(TaskNode {
            let mut base_node = graph.add_node(TaskNode {
                output: None,
                task: identity.clone(),
                phantom: std::marker::PhantomData,
                dependencies: Box::new((Arc::new(TaskInput::from("George".to_string())),)),
            });

            // let mut parent1 = Arc::new(TaskNode {
            let mut parent1_node = graph.add_node(TaskNode {
                output: None,
                task: identity.clone(),
                phantom: std::marker::PhantomData,
                dependencies: Box::new((base_node.clone(),)),
            });

            // let mut parent2 = Arc::new(TaskNode {
            //     output: None,
            //     task: identity.clone(),
            //     phantom: std::marker::PhantomData,
            //     dependencies: Box::new((base.clone(),)),
            // });

            // let dependencies = Box::new((parent1.clone(), parent2.clone()));
            // let mut combine = Arc::new(TaskNode {
            //     output: None,
            //     task: combine.clone(),
            //     phantom: std::marker::PhantomData,
            //     dependencies: dependencies.clone(),
            // });

            // let mut scheduler = Scheduler::default();
            // scheduler.add_task(combine);

            // dbg!(&scheduler);

            // render_graph(
            //     &scheduler,
            //     &PathBuf::from(file!())
            //         .parent()
            //         .unwrap()
            //         .join("../graphs/basic.svg"),
            // )?;

            // write the scheduler loop
            Ok(())
        }
    }

    task!(Task1: T1);
    task!(Task2: T1, T2);
    task!(Task3: T1, T2, T3);

    generics! {
        T1,
        T2
        // T3,
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
}
