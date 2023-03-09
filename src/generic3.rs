mod warp {
    use async_trait::async_trait;
    use by_address::ByThinAddress;
    use futures::future::BoxFuture;
    use futures::stream::{FuturesUnordered, StreamExt};
    use std::collections::{HashMap, HashSet};
    use std::future::Future;
    use std::hash::{Hash, Hasher};
    use std::pin::Pin;
    use std::sync::{Arc, Weak};
    // use tokio::sync::RwLock;
    use std::sync::RwLock;

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

    #[derive(Debug)]
    pub enum State<I, O>
// , T>
    // where
    //     T: Task<I, O> + std::fmt::Debug,
    {
        /// Task is pending and waiting to be run
        Pending(Box<dyn Task<I, O> + Send + Sync + 'static>),
        // Pending(T),
        /// Task is running
        Running,
        /// O could be a result type for fallible tasks,
        /// but then how do we fail everything in
        /// the dependency chain? this is a todo
        Completed(O),
        // todo: Succeeded(O), Failed(dyn Box<dyn Error>)
    }

    // call dependencies with arguments expanded
    // todo: taskNode should be internal, add task and dependencies via arguments to add_node
    // pub struct TaskNode<I, O, T>
    pub struct TaskNode<I, O>
// where
    //     T: Task<I, O> + std::fmt::Debug,
    {
        // task: T,
        name: String,
        state: RwLock<State<I, O>>,
        dependencies: Box<dyn Dependencies<I> + Send + Sync>,
        // output: Option<O>, // hidden from user
        index: usize, // hidden from user
                      // phantom: std::marker::PhantomData<I>,
    }

    // impl<I, O, T> Hash for TaskNode<I, O, T>
    impl<I, O> Hash for TaskNode<I, O>
    // where
    //     T: Task<I, O> + std::fmt::Debug,
    {
        fn hash<H: Hasher>(&self, state: &mut H) {
            // hash the index
            self.index.hash(state);
        }
    }

    // impl<I, O, T> std::fmt::Debug for TaskNode<I, O, T>
    impl<I, O> std::fmt::Debug for TaskNode<I, O>
    where
        I: std::fmt::Debug,
        O: std::fmt::Debug,
        // where
        //     T: Task<I, O> + std::fmt::Debug,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // std::fmt::Debug::fmt(&self.task, f)
            // write!(f, "TaskNode({:?})", self.task)
            write!(
                f,
                "TaskNode#{}({}({:?}))",
                self.index,
                self.name,
                self.state.read().unwrap()
            )
        }
    }

    #[derive(Default, Debug)]
    pub struct TaskGraph {
        dependencies: HashMap<
            Arc<dyn Schedulable>,
            // ByThinAddress<Arc<dyn Schedulable>>,
            // HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
            HashSet<Arc<dyn Schedulable>>,
        >,
        dependants: HashMap<Arc<dyn Schedulable>, HashSet<Arc<dyn Schedulable>>>,
        // dependencies: HashMap<
        //     ByThinAddress<Arc<dyn Schedulable>>,
        //     HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
        // >,
        // dependants: HashMap<
        //     ByThinAddress<Arc<dyn Schedulable>>,
        //     HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
        // >,
    }

    // #[derive(Default, Debug)]
    // pub struct TaskRef<I, O> {
    //     input: std::marker::PhantomData<I>,
    //     output: std::marker::PhantomData<O>,
    //     index: usize,
    // }

    // impl<I, O> Hash for TaskRef<I, O> {
    //     fn hash<H: Hasher>(&self, state: &mut H) {
    //         self.index.hash(state);
    //     }
    // }

    impl TaskGraph {
        // pub fn add_node(&mut self, node: TaskNode) -> TaskRef {
        // pub fn add_node(&mut self, node: impl Schedulable) -> String {
        //     "test".into()
        // }
        // pub fn add_node<I, O, T>(&mut self, node: TaskNode<I, O, T>) -> TaskRef<I, O>

        // todo: add_input which has no dependencies
        // this should return a arc reference
        // pub fn add_node<I, O, T>(
        pub fn add_node<I, O, T>(
            &mut self,
            task: T,
            dependencies: Box<dyn Dependencies<I> + Send + Sync>,
        ) -> Arc<TaskNode<I, O>>
        // ) -> Arc<TaskNode<I, O, T>>
        // ) -> TaskRef<I, O>
        where
            // T: Task<I, O> + std::fmt::Debug,
            T: Task<I, O> + Send + Sync + 'static,
            // T: Task<I, O> + std::fmt::Debug + Send + Sync + 'static,
            // T: Task<I, O> + std::fmt::Debug,
            // I: Send + Sync,
            // O: Send + Sync,
            I: std::fmt::Debug + Send + Sync + 'static,
            O: std::fmt::Debug + Send + Sync + 'static,
        {
            let node = Arc::new(TaskNode {
                // output: None,
                // task,
                name: task.name(),
                state: RwLock::new(State::Pending(Box::new(task))),
                // phantom: std::marker::PhantomData,
                dependencies,
                index: self.dependencies.len(),
            });

            // check for circles here
            let mut seen: HashSet<Arc<dyn Schedulable>> = HashSet::new();
            let mut stack: Vec<Arc<dyn Schedulable>> = vec![node.clone()];

            // let mut seen: HashSet<ByThinAddress<Arc<dyn Schedulable>>> = HashSet::new();
            // let mut seen: HashSet<ByThinAddress<Arc<dyn Schedulable>>> = HashSet::new();
            // let mut stack: Vec<ByThinAddress<Arc<dyn Schedulable>>> = vec![ByThinAddress(node_ref)];
            while let Some(node) = stack.pop() {
                if !seen.insert(node.clone()) {
                    continue;
                }
                let mut dependencies = self
                    .dependencies
                    .entry(node.clone())
                    .or_insert(HashSet::new());

                for dep in node.dependencies() {
                    let mut dependants = self
                        .dependants
                        // .entry(ByThinAddress(dep.clone()))
                        .entry(dep.clone())
                        .or_insert(HashSet::new());
                    // Arc::downgrade(&a)
                    // dependants.insert(Arc::downgrade(&node));
                    dependants.insert(node.clone());
                    dependencies.insert(dep.clone());
                    // dependencies.insert(ByThinAddress(dep.clone()));
                    // stack.push(ByThinAddress(dep));
                    // dependencies are task nodes already, so they already have an index
                    stack.push(dep);
                }
            }

            // let mut dependencies = self
            //     .dependencies
            //     .entry(task.clone())
            //     .or_insert(HashSet::new());

            // self.dependencies
            // create a task ref
            // let node_idx = NodeIndex::new(self.nodes.len());
            // NodeIndex::new(self.nodes.len());
            // TaskRef {
            //     input: std::marker::PhantomData,
            //     output: std::marker::PhantomData,
            //     index,
            // }
            node
        }
    }

    // #[derive(Default, Debug)]
    // pub struct Scheduler {
    //     dependencies: HashMap<
    //         ByThinAddress<Arc<dyn Schedulable>>,
    //         HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
    //     >,
    //     dependants: HashMap<
    //         ByThinAddress<Arc<dyn Schedulable>>,
    //         HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
    //     >,
    // }

    // impl Scheduler {
    //     // pub fn add_task(&mut self, task: impl TaskNode<I, O>) {
    //     pub fn add_task<I, O, T>(&mut self, task: Arc<TaskNode<I, O, T>>)
    //     where
    //         T: Task<I, O> + std::fmt::Debug + Send + Sync + 'static,
    //         I: std::fmt::Debug + Send + Sync + 'static,
    //         O: std::fmt::Debug + Send + Sync + 'static,
    //     {
    //         // sanity check that partial eq works
    //         assert_eq!(ByThinAddress(task.clone()), ByThinAddress(task.clone()));

    //         // sanity check that hash works
    //         use std::hash::Hash;
    //         let hasher = std::collections::hash_map::DefaultHasher::new();
    //         assert_eq!(
    //             ByThinAddress(task.clone()).hash(&mut hasher.clone()),
    //             ByThinAddress(task.clone()).hash(&mut hasher.clone())
    //         );

    //         // check for circles here
    //         let mut seen: HashSet<ByThinAddress<Arc<dyn Schedulable>>> = HashSet::new();
    //         let mut stack: Vec<ByThinAddress<Arc<dyn Schedulable>>> = vec![ByThinAddress(task)];
    //         while let Some(task) = stack.pop() {
    //             if !seen.insert(task.clone()) {
    //                 continue;
    //             }
    //             let mut dependencies = self
    //                 .dependencies
    //                 .entry(task.clone())
    //                 .or_insert(HashSet::new());

    //             for dep in task.dependencies() {
    //                 let mut dependants = self
    //                     .dependants
    //                     .entry(ByThinAddress(dep.clone()))
    //                     .or_insert(HashSet::new());
    //                 dependants.insert(task.clone());
    //                 dependencies.insert(ByThinAddress(dep.clone()));
    //                 stack.push(ByThinAddress(dep));
    //             }
    //         }
    //     }
    // }

    // we cannot just keep using the TaskNode, because we need to often mix and match them and
    // they all have different generic parameters
    #[async_trait]
    pub trait Schedulable {
        /// is ready for execution
        fn ready(&self) -> bool;

        /// task has completed
        fn completed(&self) -> bool;

        /// index
        fn index(&self) -> usize;

        /// run the task
        async fn run(&self);
        // async fn run(&mut self);

        // /// redirect format
        // /// todo: this seems hacky, remove?
        // fn debug(&self) -> &dyn std::fmt::Debug;

        /// get the dependencies
        fn dependencies(&self) -> Vec<Arc<dyn Schedulable>>;
    }

    impl std::fmt::Debug for dyn Schedulable {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(self.debug(), f)
        }
    }

    // impl Hash for dyn Schedulable + '_ {
    impl Hash for dyn Schedulable {
        #[inline]
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.index().hash(state);
        }
    }

    // impl PartialEq for dyn Schedulable + '_ {
    impl PartialEq for dyn Schedulable {
        #[inline]
        fn eq(&self, other: &Self) -> bool {
            std::cmp::PartialEq::eq(&self.index(), &other.index())
        }
    }

    impl Eq for dyn Schedulable {}
    // impl Eq for dyn Schedulable + '_ {}

    // task input should really just be a shorthand for a special task we define that is generic
    // over the return type, so it can be indexed as well etc.
    // #[async_trait]
    // impl<O> Schedulable for TaskInput<O>
    // where
    //     O: Sync + Send + std::fmt::Debug,
    // {
    //     fn ready(&self) -> bool {
    //         true
    //     }

    //     fn completed(&self) -> bool {
    //         true
    //     }

    //     async fn run(&mut self) {
    //         // do nothing
    //     }

    //     fn debug(&self) -> &dyn std::fmt::Debug {
    //         self
    //     }

    //     fn dependencies(&self) -> Vec<Arc<dyn Schedulable>> {
    //         vec![]
    //     }
    // }

    impl<O> std::fmt::Debug for TaskInput<O>
    where
        O: std::fmt::Debug,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TaskInput({:?})", &self.value)
        }
    }

    #[async_trait]
    // impl<I, O, T> Schedulable for TaskNode<I, O, T>
    impl<I, O> Schedulable for TaskNode<I, O>
    where
        // T: Task<I, O> + std::fmt::Debug + Send + Sync,
        I: std::fmt::Debug + Send + Sync,
        O: std::fmt::Debug + Send + Sync,
    {
        /// a task is ready if its dependencies have completed
        fn ready(&self) -> bool {
            self.dependencies().iter().all(|d| d.completed())
        }

        /// a task is completed if its output is available
        fn completed(&self) -> bool {
            match *self.state.read().unwrap() {
                State::Completed(_) => true,
                _ => false,
            }
            // self.output.is_some()
        }

        /// index of the task node in the DAG graph
        fn index(&self) -> usize {
            self.index
        }

        // async fn run(&mut self) {
        async fn run(&self) {
            // get the inputs from the dependencies
            let inputs = self.dependencies.inputs().unwrap();
            println!("running task {:?}({:?})", self, inputs);
            let state = {
                let state = &mut *self.state.write().unwrap();
                if let State::Pending(task) = state {
                    std::mem::replace(state, State::Running)
                } else {
                    // already done
                    return;
                }
            };
            // let output = pending.run(inputs).await;
            // let state = std::mem::replace(&mut state, State::Running);
            // match state {
            if let State::Pending(mut task) = state {
                let output = task.run(inputs).await;
                let state = &mut *self.state.write().unwrap();
                *state = State::Completed(output);
                // std::mem::replace(state, State::Running)
            }
            // _ => {}
            // }
            // self.output = Some();
        }

        // fn debug(&self) -> &dyn std::fmt::Debug {
        //     self
        // }

        fn dependencies(&self) -> Vec<Arc<dyn Schedulable>> {
            self.dependencies.to_vec()
        }
    }

    pub trait Dependency<O>: Schedulable {
        // fn output(&self) -> Option<&O>;
        fn output(&self) -> Option<O>;
    }

    // impl<O> Dependency<O> for TaskInput<O>
    // where
    //     O: Sync + Send + std::fmt::Debug + 'static,
    // {
    //     fn output(&self) -> Option<&O> {
    //         Some(&self.value)
    //     }
    // }

    // impl<I, O, T> Dependency<O> for TaskNode<I, O, T>
    impl<I, O> Dependency<O> for TaskNode<I, O>
    where
        // T: Task<I, O> + std::fmt::Debug + Send + Sync + 'static,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Clone + Send + Sync + 'static,
    {
        // fn output(&self) -> Option<&O> {
        fn output(&self) -> Option<O> {
            match &*self.state.read().unwrap() {
                // State::Completed(ref output) => Some(output),
                State::Completed(output) => Some(output.clone()),
                _ => None,
            }
            // self.output.as_ref()
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
                (Some(i1),) => Some((i1.clone(),)),
                _ => None,
            }
        }
    }

    // two dependencies etc.
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

        // todo: arc clone the outputs? so we do not impose a clone bound on outputs (and inputs
        // ...)
        fn inputs(&self) -> Option<(T1, T2)> {
            let (i1, i2) = self;
            match (i1.output(), i2.output()) {
                (Some(i1), Some(i2)) => Some((i1.clone(), i2.clone())),
                _ => None,
            }
        }
    }

    #[async_trait]
    pub trait Task<I, O>: std::fmt::Debug {
        async fn run(&mut self, input: I) -> O;

        fn name(&self) -> String {
            format!("{:?}", self)
        }
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
    impl<O> Task<(), O> for TaskInput<O>
    where
        O: std::fmt::Debug + Clone + Send,
    {
        async fn run(&mut self, input: ()) -> O {
            println!("task input {:?}", &self.value);
            self.value.clone()
        }
    }

    // #[async_trait]
    // pub trait TaskNew {
    //     type Input;
    //     type Output;

    //     async fn run(&mut self, input: Self::Input) -> Self::Output;
    // }

    // #[async_trait]
    // pub trait ManualTask1<T1, O> {
    //     async fn run(&mut self, t1: T1) -> O;
    // }

    // #[async_trait]
    // pub trait ManualTask2<T1, T2, O> {
    //     async fn run(&mut self, t1: T1, t2: T2) -> O;
    // }

    macro_rules! task {
        ($name:ident: $( $type:ident ),*) => {
            #[async_trait]
            pub trait $name<$( $type ),*, O>: std::fmt::Debug {
                async fn run(&mut self, $($type: $type),*) -> O;
            }
            #[async_trait]
            impl<T, $( $type ),*, O> Task<($( $type ),*,), O> for T
            where
                T: $name<$( $type ),*, O> + Send + 'static,
                $($type: std::fmt::Debug + Send + 'static),*
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

        // fn render_scheduler(scheduler: &Scheduler, path: impl AsRef<Path>) -> Result<()> {
        //     use layout::backends::svg::SVGWriter;
        //     use layout::core::{self, base::Orientation, color::Color, style};
        //     use layout::gv::GraphBuilder;
        //     use layout::std_shapes::shapes;
        //     use layout::topo::layout::VisualGraph;
        //     use std::io::{BufWriter, Write};

        //     let mut graph = VisualGraph::new(Orientation::TopToBottom);

        //     fn node(node: &ByThinAddress<Arc<dyn Schedulable>>) -> shapes::Element {
        //         let node_style = style::StyleAttr {
        //             line_color: Color::new(0x000000FF),
        //             line_width: 2,
        //             fill_color: Some(Color::new(0xB4B3B2FF)),
        //             rounded: 0,
        //             font_size: 15,
        //         };
        //         let size = core::geometry::Point { x: 100.0, y: 100.0 };
        //         shapes::Element::create(
        //             shapes::ShapeKind::Circle(format!("{:?}", node.0.debug())),
        //             node_style.clone(),
        //             Orientation::TopToBottom,
        //             size,
        //         )
        //     }

        //     let mut handles: HashMap<
        //         ByThinAddress<Arc<dyn Schedulable>>,
        //         layout::adt::dag::NodeHandle,
        //     > = HashMap::new();
        //     for (task, deps) in &scheduler.dependencies {
        //         let dest_handle = *handles
        //             .entry(task.clone())
        //             .or_insert_with(|| graph.add_node(node(task)));
        //         for dep in deps {
        //             let src_handle = *handles
        //                 .entry(dep.clone())
        //                 .or_insert_with(|| graph.add_node(node(dep)));
        //             let arrow = shapes::Arrow {
        //                 start: shapes::LineEndKind::None,
        //                 end: shapes::LineEndKind::Arrow,
        //                 line_style: style::LineStyleKind::Normal,
        //                 text: "".to_string(),
        //                 look: style::StyleAttr {
        //                     line_color: Color::new(0x000000FF),
        //                     line_width: 2,
        //                     fill_color: Some(Color::new(0xB4B3B2FF)),
        //                     rounded: 0,
        //                     font_size: 15,
        //                 },
        //                 src_port: None,
        //                 dst_port: None,
        //             };
        //             graph.add_edge(arrow, src_handle, dest_handle);
        //         }
        //     }

        //     let mut backend = SVGWriter::new();
        //     let debug_mode = false;
        //     let disable_opt = false;
        //     let disable_layout = false;
        //     graph.do_it(debug_mode, disable_opt, disable_layout, &mut backend);
        //     let content = backend.finalize();

        //     let file = std::fs::OpenOptions::new()
        //         .write(true)
        //         .truncate(true)
        //         .create(true)
        //         .open(path.as_ref())?;
        //     let mut writer = BufWriter::new(file);
        //     writer.write_all(content.as_bytes());
        //     Ok(())
        // }

        fn render_graph(task_graph: &TaskGraph, path: impl AsRef<Path>) -> Result<()> {
            use layout::backends::svg::SVGWriter;
            use layout::core::{self, base::Orientation, color::Color, style};
            use layout::gv::GraphBuilder;
            use layout::std_shapes::shapes;
            use layout::topo::layout::VisualGraph;
            use std::io::{BufWriter, Write};

            let mut graph = VisualGraph::new(Orientation::TopToBottom);

            // fn node(node: &ByThinAddress<Arc<dyn Schedulable>>) -> shapes::Element {
            fn node(node: &Arc<dyn Schedulable>) -> shapes::Element {
                let node_style = style::StyleAttr {
                    line_color: Color::new(0x000000FF),
                    line_width: 2,
                    fill_color: Some(Color::new(0xB4B3B2FF)),
                    rounded: 0,
                    font_size: 15,
                };
                let size = core::geometry::Point { x: 100.0, y: 100.0 };
                shapes::Element::create(
                    shapes::ShapeKind::Circle(format!("{:?}", node.debug())),
                    node_style.clone(),
                    Orientation::TopToBottom,
                    size,
                )
            }

            let mut handles: HashMap<
                Arc<dyn Schedulable>,
                // Arc<dyn Schedulable>,
                // ByThinAddress<Arc<dyn Schedulable>>,
                layout::adt::dag::NodeHandle,
            > = HashMap::new();

            for (task, deps) in &task_graph.dependencies {
                let dest_handle = *handles
                    .entry(task.clone())
                    .or_insert_with(|| graph.add_node(node(task)));
                for dep in deps {
                    let src_handle = *handles
                        // .entry(Arc::downgrade(dep))
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
            // the trait pendency<String>
            // Dependency<String> for warp::TaskInput<String> ?
            // if we allow the dependencies to not be added yet, we could end up with cirles etc.
            // lets not do that?
            let mut input_node =
                graph.add_node(TaskInput::from("George".to_string()), Box::new(()));

            let mut base_node = graph.add_node(identity.clone(), Box::new((input_node,)));
            let mut parent1_node = graph.add_node(identity.clone(), Box::new((base_node.clone(),)));
            let mut parent2_node = graph.add_node(identity.clone(), Box::new((base_node.clone(),)));
            let mut combine_node =
                graph.add_node(combine.clone(), Box::new((parent1_node, parent2_node)));
            dbg!(&graph);

            // let mut base_node = graph.add_node(TaskNode {
            //     output: None,
            //     task: identity.clone(),
            //     phantom: std::marker::PhantomData,
            //     dependencies: Box::new((Arc::new(TaskInput::from("George".to_string())),)),
            // });

            // let mut parent1 = Arc::new(TaskNode {
            // let mut parent1_node = graph.add_node(identity.clone(), Box::new((base_node.clone(),)));
            // let mut parent1_node = graph.add_node(TaskNode {
            //     output: None,
            //     task: identity.clone(),
            //     phantom: std::marker::PhantomData,
            //     dependencies: Box::new((base_node.clone(),)),
            // });

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

            render_graph(
                &graph,
                &PathBuf::from(file!())
                    .parent()
                    .unwrap()
                    .join("../graphs/basic.svg"),
            )?;

            // write the scheduler loop
            type TaskFut = dyn Future<Output = Result<Arc<dyn Schedulable>>>;
            let mut tasks: FuturesUnordered<Pin<Box<TaskFut>>> = FuturesUnordered::new();

            // let mut ready: Vec<Arc<Mutex<dyn Schedulable>>> = graph
            // let mut ready: Vec<&Arc<dyn Schedulable>> = graph
            let mut ready: Vec<Arc<dyn Schedulable>> = graph
                .dependencies
                // .iter()
                .keys()
                .filter(|t| t.ready())
                // .map(|t| t.0.clone())
                .cloned()
                // .filter_map(|(t, deps)| {
                //     if deps.is_empty() {
                //         Some(t.0.clone())
                //     } else {
                //         None
                //     }
                // })
                .collect();
            dbg!(&ready);
            // // p.ready()).cloned().collect();

            loop {
                // check if we are done
                if tasks.is_empty() && ready.is_empty() {
                    println!("we are done");
                    break;
                }

                // start running ready tasks
                for p in ready.drain(0..) {
                    tasks.push(Box::pin(async move {
                        // let inputs = p.task.dependencies.inputs().unwrap();
                        // let output = p.task.run(inputs).await;
                        println!("running {:?}", &p);
                        p.run().await;
                        Ok(p)
                    }));
                }

                // wait for a task to complete
                match tasks.next().await {
                    Some(Err(err)) => {
                        println!("task failed {:?}", &err);
                        anyhow::bail!("a task failed: {}", err)
                    }
                    Some(Ok(completed)) => {
                        println!("task completed {:?}", &completed);

                        // update ready tasks
                        // there was no reverse link set...
                        // either handle that here or add empty set?

                        // let empty = HashSet::new();
                        // let dependants = &graph.dependants.get(&completed).unwrap_or(&empty); //(&vec![]);
                        if let Some(dependants) = &graph.dependants.get(&completed) {
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
                    None => {}
                }
                // break;
            }

            // check the graph now
            dbg!(&graph);
            assert!(false);

            Ok(())
        }
    }

    // task!(Task0: ());
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
