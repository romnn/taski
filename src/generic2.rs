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

    // #[derive(Debug)]
    pub struct TaskNode<I, O, T>
    where
        T: Task<I, O> + std::fmt::Debug,
    {
        // do not return a future, make this function the future itself?
        // async ...
        // fn run(&mut self, input: I) -> Pin<Box<dyn Future<Output = O> + Send + Sync>>;
        task: T,
        // task: Arc<T>,
        // task: Box<dyn Task<I, O>>,
        // dependencies: &dyn Dependencies<I>,
        dependencies: Box<dyn Dependencies<I> + Send + Sync>,
        output: Option<O>,
        phantom: std::marker::PhantomData<I>,
        // dependencies: Box<dyn Dependencies<I>>,
        // dependencies: Dependencies<I>,
        // pointers to dyn tasks? => how to get the outputs then

        // Box<dyn Tuple<Tuple=I>>,
        // dependencies: Box<dyn IntoProduct<Tuple=I>>,
        // dependencies: Box<dyn Dependencies<I>>,
        // async fn run(&mut self, input: I) -> O;
        // fn dependencies(&self) -> Vec<Dependency<I>; // vec
    }

    impl<I, O, T> std::fmt::Debug for TaskNode<I, O, T>
    where
        T: Task<I, O> + std::fmt::Debug,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // write!(
            // std::fmt::Debug::fmt(&self.0, f)
            std::fmt::Debug::fmt(&self.task, f)
            // f.debug_struct("TaskNode")
            //     .field("input", &std::any::type_name::<I>())
            //     .field("output", &std::any::type_name::<O>())
            //     .finish()
        }
    }

    #[derive(Default, Debug)]
    pub struct Scheduler {
        // tasks: Vec<Box<dyn Schedulable>>,
        // dependencies: HashMap<Box<dyn Schedulable>, Box<dyn Schedulable>>,
        // dependants: HashMap<Arc<dyn Schedulable>, Arc<dyn Schedulable>>,
        // dependants: HashMap<Arc<Box<dyn Schedulable>>, usize>,
        // todo: use weak pointers from dependencies to dependants
        // dependencies: HashMap<ScheduleItem, HashSet<ScheduleItem>>,
        dependencies: HashMap<
            ByThinAddress<Arc<dyn Schedulable>>,
            HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
        >,
        // Arc<dyn Schedulable>>,
        dependants: HashMap<
            ByThinAddress<Arc<dyn Schedulable>>,
            HashSet<ByThinAddress<Arc<dyn Schedulable>>>,
        >,
        // dependants: HashMap<ScheduleItem, HashSet<ScheduleItem>>,
        // Arc<dyn Schedulable>>,
        // Arc<Box<dyn Schedulable>>>,
        // pub pool: FuturesUnordered<Pin<Box<dyn Task>>>,
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

            // self.tasks.push(Box::new(task));
            // check for circles here
            let mut seen: HashSet<ByThinAddress<Arc<dyn Schedulable>>> = HashSet::new();
            let mut stack: Vec<ByThinAddress<Arc<dyn Schedulable>>> = vec![ByThinAddress(task)];
            while let Some(task) = stack.pop() {
                if !seen.insert(task.clone()) {
                    continue;
                }
                // seen.insert(ByThinAddress(dep.clone()));
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
            // for dep in &task.dependencies.to_vec() {
            //     let mut dependants = self
            //         .dependants
            //         .entry(ByThinAddress(dep.clone()))
            //         .or_insert(HashSet::new());
            //     dependants.insert(ByThinAddress(task.clone()));
            //     // self.dependants.insert(, );
            //     let mut dependencies = self
            //         .dependencies
            //         .entry(ByThinAddress(task.clone()))
            //         .or_insert(HashSet::new());
            //     dependencies.insert(ByThinAddress(dep.clone()));
            // }
            // self.dependencies
            //     .insert(Box::new(task), task.dependencies.map(Box::new));
        }
    }

    // #[derive(Debug)]
    // struct RefEquality<'a, T>(&'a T);

    // impl<'a, T> std::hash::Hash for RefEquality<'a, T> {
    //     fn hash<H>(&self, state: &mut H)
    //     where
    //         H: std::hash::Hasher,
    //     {
    //         (self.0 as *const T).hash(state)
    //     }
    // }

    // impl<'a, 'b, T> PartialEq<RefEquality<'b, T>> for RefEquality<'a, T> {
    //     fn eq(&self, other: &RefEquality<'b, T>) -> bool {
    //         self.0 as *const T == other.0 as *const T
    //     }
    // }

    // impl<'a, T> Eq for RefEquality<'a, T> {}

    // #[derive(Hash, PartialEq, Eq)]
    // type ScheduleItemInner = Arc<Box<dyn Schedulable>>;
    // type ScheduleItemInner = Arc<dyn Schedulable>;

    // #[derive(Clone)]
    // pub struct ScheduleItem(ScheduleItemInner);

    // impl std::fmt::Debug for ScheduleItem {
    //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    //         // write!(
    //         std::fmt::Debug::fmt(&self.0, f)
    //         // f.debug_struct("Schedulable")
    //         //     .field("ready", &self.ready())
    //         //     .finish()
    //     }
    // }

    // pub struct ScheduleItem {
    //     inner:
    // }

    // impl std::hash::Hash for ScheduleItem {
    //     fn hash<H>(&self, state: &mut H)
    //     where
    //         H: std::hash::Hasher,
    //     {
    //         Arc::as_ptr(&self.0).hash(state)
    //         // (self.0 as *const T).hash(state)
    //     }
    // }

    // impl PartialEq<ScheduleItem> for ScheduleItem {
    //     fn eq(&self, other: &ScheduleItem) -> bool {
    //         Arc::ptr_eq(&self.0, &other.0)
    //         // self.0 as *const T == other.0 as *const T
    //     }
    // }

    // impl Eq for ScheduleItem {}
    // pub trait Schedulable: std::hash::Hash + Eq {

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
            // Schedulable::fmt(self, f)
            // f.debug_struct("Schedulable")
            //     .field("ready", &self.ready())
            //     .finish()
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
        // fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //     self)
        // }
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
            // std::fmt::Debug::fmt(&self.0, f)
            // f.debug_struct("TaskInput")
            //     .field("value", &self.value)
            //     .finish()
        }
    }

    #[async_trait]
    // impl<I, O, T> Schedulable for Arc<TaskNode<I, O, T>>
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

        // async fn run(self: Arc<Self>) {
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
            // vec![]
        }
    }

    pub trait Dependency<O>: Schedulable {
        fn output(&self) -> Option<&O>;
        // fn test(self: Arc<Self>) -> Arc<dyn Schedulable>;
        // fn test(&self) -> Arc<dyn Schedulable>;
    }

    // impl<O> Dependency<O> for O {
    //     fn output(&self) -> Option<&O> {
    //         Some(self)
    //     }
    // }

    impl<O> Dependency<O> for TaskInput<O>
    where
        O: Sync + Send + std::fmt::Debug + 'static,
    {
        fn output(&self) -> Option<&O> {
            Some(&self.value)
        }
        // fn test(self: Arc<Self>) -> Arc<dyn Schedulable> {
        //     self as Arc<dyn Schedulable>
        //     // Arc::new(*self)
        // }
    }

    // impl<I, O, T> Dependency<O> for Arc<TaskNode<I, O, T>>
    impl<I, O, T> Dependency<O> for TaskNode<I, O, T>
    where
        T: Task<I, O> + std::fmt::Debug + Send + Sync + 'static,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
    {
        fn output(&self) -> Option<&O> {
            self.output.as_ref()
        }
        // fn test(self: Arc<Self>) -> Arc<dyn Schedulable> {
        //     self as Arc<dyn Schedulable>
        // }
    }

    pub trait Dependencies<O> {
        fn to_vec(&self) -> Vec<Arc<dyn Schedulable>>;
        fn inputs(&self) -> Option<O>; // O = (T1,)
    }

    impl<D1, T1> Dependencies<(T1,)> for (Arc<D1>,)
    where
        D1: Dependency<T1> + 'static,
        T1: Clone,
    {
        fn to_vec(&self) -> Vec<Arc<dyn Schedulable>> {
            // vec![self.0 as Arc<dyn Schedulable>]
            vec![self.0.clone() as Arc<dyn Schedulable>]
            // vec![self.0.clone().test()]
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

    // impl<I1, T1> Dependencies<(T1,)> for (TaskNode<I1, T1>,) {}
    // impl<I1, I2, T1, T2> Dependencies<(T1, T2)> for (TaskNode<I1, T1>, TaskNode<I2, T2>) {}

    // impl<T1> Dependencies<(T1,)> for (T1,) {}
    // impl<T1, T2> Dependencies<(T1, T2)> for (T1, T2) {}

    // where I: Product or Tuple
    #[async_trait]
    pub trait Task<I, O> {
        // should we use associated types here? e.g. for output
        // do not return a future, make this function the future itself?
        // async ...
        // fn run(&mut self, input: I) -> Pin<Box<dyn Future<Output = O> + Send + Sync>>;
        async fn run(&mut self, input: I) -> O;
        // fn dependencies(&self) -> Vec<Dependency<I>; // vec
    }

    pub struct TaskInput<O> {
        value: O,
    }

    impl<O> From<O> for TaskInput<O> {
        fn from(value: O) -> Self {
            Self { value }
        }
    }

    // pub struct Input<O> {
    //     value: O,
    // }

    // #[async_trait]
    // impl<O> Task<(), O> for Input<O> {
    //     async fn run(&mut self, input: ()) -> O {
    //         self.value
    //     }
    // }

    // impl<O> From<O> for Input<O> {
    //     fn from(value: O) -> Self {
    //         Self { value }
    //     }
    // }

    // impl<O> Into<Input<O>> for O {
    //     fn into(value: O) -> Input<O> {
    //         Input { value }
    //     }
    // }

    // impl<I, T, O> Dependency<O> for T where T: Task<I, O> {}

    // where I: Product or Tuple
    #[async_trait]
    pub trait TaskNew {
        type Input;
        type Output;
        // should we use associated types here? e.g. for output
        // do not return a future, make this function the future itself?
        // async ...
        // fn run(&mut self, input: I) -> Pin<Box<dyn Future<Output = O> + Send + Sync>>;
        async fn run(&mut self, input: Self::Input) -> Self::Output;
        // fn dependencies(&self) -> Vec<Dependency<I>; // vec
    }

    // impl<T, O> Dependency<O> for T where T: TaskNew<Output=O> {}

    // #[async_trait]
    // pub trait NewManualTask2<T1, T2, O> {
    //     async fn run(&mut self, t1: T1, t2: T2) -> O;
    // }

    // #[async_trait]
    // // impl<T, T1, T2, O> TaskNew<Input=(T1, T2,)> for T
    // // impl<T, T1, T2, O> TaskNew for T
    // impl<T, T1, T2, O> TaskNew for T
    // where
    //     // I: IntoTuple<Tuple = (T1,)> + Send + 'async_trait,
    //     // T::Input: ,
    //     // T::Output: O,
    //     T: NewManualTask2<T1, T2, O> + Send + 'static,
    //     T1: Send + 'static,
    //     T2: Send + 'static,
    // {
    //     type Output = O;
    //     type Input = (T1, T2);

    //     async fn run(&mut self, input: (T1,T2,)) -> O {
    //         // destructure to tuple and call
    //         let (t1,t2) = input; // .flatten();
    //         NewManualTask2::run(self, t1, t2).await
    //     }
    // }

    #[async_trait]
    pub trait ManualTask1<T1, O> {
        async fn run(&mut self, t1: T1) -> O;
    }

    #[async_trait]
    pub trait ManualTask2<T1, T2, O> {
        async fn run(&mut self, t1: T1, t2: T2) -> O;
    }

    // #[async_trait]
    // impl<T, T1, O> Task<(T1,), O> for T
    // where
    //     // I: IntoTuple<Tuple = (T1,)> + Send + 'async_trait,
    //     T: ManualTask1<T1, O> + Send + 'static,
    //     T1: Send + 'static,
    // {
    //     async fn run(&mut self, input: (T1,)) -> O {
    //         // destructure to tuple and call
    //         let (t1,) = input; // .flatten();
    //         ManualTask1::run(self, t1).await
    //     }
    // }

    // #[async_trait]
    // impl<T, T1, T2, O> Task<(T1, T2), O> for T
    // where
    //     // I: IntoTuple<Tuple = (T1,)> + Send + 'async_trait,
    //     T: ManualTask2<T1, T2, O> + Send + 'static,
    //     T1: Send + 'static,
    //     T2: Send + 'static,
    // {
    //     async fn run(&mut self, input: (T1, T2)) -> O {
    //         // destructure to tuple and call
    //         let (t1, t2) = input; // .flatten();
    //         ManualTask2::run(self, t1, t2).await
    //     }
    // }

    // impl<I, T, T1, O> Task<I, O> for T
    // where
    //     I: IntoTuple<Tuple = (T1)> + Send + 'async_trait,
    //     T: Task1<T1, O> + Send,
    //     T1: Send,
    // {
    //     async fn run(&mut self, input: I) -> O {
    //         // destructure to tuple and call
    //         let (t1) = input.flatten();
    //         Task1::run(self, t1).await
    //     }
    // }

    // then, we need pub trait Task1<(A, B), O> { ...
    // impl Task<I, O> for Task1<...

    // pub trait Func<Args> {
    //     type Output;

    //     fn call(&self, args: Args) -> Self::Output;
    // }

    // #[async_trait]
    // pub trait Task<A, B, O> {
    //     // do not return a future, make this function the future itself?
    //     // async ...
    //     // fn run(&mut self, input: I) -> Pin<Box<dyn Future<Output = O> + Send + Sync>>;
    //     // async fn run(&mut self, a: A, b: B) -> O;
    //     // fn dependencies(&self) -> Vec<Dependency<I>; // vec
    // }

    // download track: none
    // download artwork: none
    // convert track to high quality mpeg: dependency: downloaded track
    // convert track to low quality wav: dependency: downloaded track
    // embed artwork: dependency: artwork download
    // run matching: dependencies: top 5 tracks

    // impl<F, R> Func<()> for F
    // where
    //     F: Fn() -> R,
    // {
    //     type Output = R;

    //     #[inline]
    //     fn call(&self, _args: ()) -> Self::Output {
    //         (*self)()
    //     }
    // }

    // impl<F, R> Func<crate::Rejection> for F
    // where
    //     F: Fn(crate::Rejection) -> R,
    // {
    //     type Output = R;

    //     #[inline]
    //     fn call(&self, arg: crate::Rejection) -> Self::Output {
    //         (*self)(arg)
    //     }
    // }

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
                // T1: Send + 'static,
                // T2: Send + 'static,
            {
                async fn run(&mut self, input: ($( $type ),*,)) -> O {
                    // destructure to tuple and call
                    let ($( $type ),*,) = input; // .flatten();
                    $name::run(self, $( $type ),*).await
                }
            }
        }
    }

    // macro_rules! tasks {
    //     ($type:ident) => {
    //         #[async_trait]
    //         pub trait Task1<T1, O> {
    //             async fn run(&mut self, t1: T1) -> O;
    //         }
    //     };
    //     ($type1:ident, $( $type:ident ),*) => {
    //         tasks!($( $type ),*);
    //         #[async_trait]
    //         pub trait Task2<$type1, $( $type ),*, O> {
    //             async fn run(&mut self, $($type: $type),*) -> O;
    //         }
    //     }
    // }

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

        // impl<F, R, $type> Func<Product!($type)> for F
        // where
        //     F: Fn($type) -> R,
        // {
        //     type Output = R;

        //     #[inline]
        //     fn call(&self, args: Product!($type)) -> Self::Output {
        //         (*self)(args.0)
        //     }

        // }

        // impl<F, R, $type> Func<($type,)> for F
        // where
        //     F: Fn($type) -> R,
        // {
        //     type Output = R;

        //     #[inline]
        //     fn call(&self, args: ($type,)) -> Self::Output {
        //         (*self)(args.0)
        //     }
        // }

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

        // impl<F, R, $type1, $( $type ),*> Func<Product!($type1, $($type),*)> for F
        // where
        //     F: Fn($type1, $( $type ),*) -> R,
        // {
        //     type Output = R;

        //     #[inline]
        //     fn call(&self, args: Product!($type1, $($type),*)) -> Self::Output {
        //         #[allow(non_snake_case)]
        //         let product_pat!($type1, $( $type ),*) = args;
        //         (*self)($type1, $( $type ),*)
        //     }
        // }

        // impl<F, R, $type1, $( $type ),*> Func<($type1, $($type),*)> for F
        // where
        //     F: Fn($type1, $( $type ),*) -> R,
        // {
        //     type Output = R;

        //     #[inline]
        //     fn call(&self, args: ($type1, $($type),*)) -> Self::Output {
        //         #[allow(non_snake_case)]
        //         let ($type1, $( $type ),*) = args;
        //         (*self)($type1, $( $type ),*)
        //     }
        // }
    };
}

    // pub struct TaskNode<F, I, O>
    // where
    //     // todo: make this the function trait that can be called with either a product or a tuple of
    //     // the correct arguments
    //     F: FnOnce(I) -> Pin<Box<dyn Future<Output = O> + Send + Sync>> + Send + Sync,
    // {
    //     func: F,
    // }

    // either: make this implement future and poll the inner future, which would allow us to keep
    // some references to the super task that could be helpful
    // pub struct InFlightTaskFut<F, O>
    pub struct InFlightTaskFut<O>
    where
        Self: Future,
    {
        fut: Pin<Box<dyn Future<Output = O> + Send + Sync>>,
        // reference back to the parent task?
    }

    // or: use the future box with O directly, no way to keep reference to the dependants that are
    // waiting for this task

    // pub struct Scheduler {
    //     // Future<Output = PoolResult<(I, Result<O, E>)>> + Send + Sync
    //     // pub pool: FuturesUnordered<Pin<Box<dyn Task>>>,
    // }

    // impl Scheduler {
    //     fn new()
    // }

    #[cfg(test)]
    mod tests {
        use super::*;
        use anyhow::Result;
        use async_trait::async_trait;
        use std::path::{Path, PathBuf};

        fn render_graph(scheduler: &Scheduler, path: impl AsRef<Path>) -> Result<()> {
            use layout::backends::svg::SVGWriter;
            use layout::core::{self, base::Orientation, color::Color, style};
            // use layout::gv::parser::DotParser;
            use layout::gv::GraphBuilder;
            use layout::std_shapes::shapes;
            use layout::topo::layout::VisualGraph;
            use std::io::{BufWriter, Write};

            // let contents = "";
            // let mut parser = DotParser::new(&contents);
            // let tree = parser.process().map_err(|err| anyhow::anyhow!(err))?;
            // let mut builder = GraphBuilder::new();
            // builder.visit_graph(&tree);
            // let mut visual_graph = builder.get();
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
            // let mut sched = Scheduler
            // type Pool = FuturesUnordered<Pin<Box<dyn InFlightTaskFutTask>>>;
            // type Pool = FuturesUnordered<InFlightTaskFutTask>;

            // let mut pool: Pool = FuturesUnordered::new();

            // struct BaseTask {
            //     name: String,
            // }

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

            // #[async_trait]
            // impl Task<(String, String), String> for Combine {
            //     async fn run(&mut self, input: (String, String)) -> String {
            //         println!("combine with input: {:?}", input);
            //         let (a, b) = input;
            //         format!("{} {}", a, b)
            //     }
            // }

            #[async_trait]
            impl Task2<String, String, String> for Combine {
                async fn run(&mut self, a: String, b: String) -> String {
                    println!("combine with input: {:?}", (&a, &b));
                    // let (a, b) = input;
                    format!("{} {}", &a, &b)
                }
            }

            // let mut base_task = SimpleTask {
            //     // name: "Roman".to_string(),
            // };

            let mut combine = Combine {};
            let mut identity = Identity {};

            // let mut please = Arc::new(Box::new(TaskNode {
            //     output: None,
            //     task: identity.clone(),
            //     phantom: std::marker::PhantomData,
            //     // task: Box::new(identity.clone()),
            //     // dependencies: &("George".to_string(),),
            //     // dependencies: Box::new(("George".to_string().into::<Input<_>>(),)),
            //     dependencies: Box::new((Arc::new(TaskInput::from("George".to_string())),)),
            //     // dependencies: Box::new(("George",)),
            // }));
            // let mut dependants: HashMap<ScheduleItem, Arc<Box<dyn Schedulable>>>;
            // dependants.insert(ScheduleItem(please as Arc<Box<), please);

            // make task nodes
            let mut base = Arc::new(TaskNode {
                output: None,
                task: identity.clone(),
                phantom: std::marker::PhantomData,
                // task: Box::new(identity.clone()),
                // dependencies: &("George".to_string(),),
                // dependencies: Box::new(("George".to_string().into::<Input<_>>(),)),
                dependencies: Box::new((Arc::new(TaskInput::from("George".to_string())),)),
                // dependencies: Box::new(("George",)),
            });

            let mut parent1 = Arc::new(TaskNode {
                output: None,
                task: identity.clone(),
                phantom: std::marker::PhantomData,
                // task: Box::new(identity.clone()),
                // dependencies: &(base,),
                dependencies: Box::new((base.clone(),)),
                // "George".to_string(),),
                // dependencies: Box::new(("George",)),
            });

            let mut parent2 = Arc::new(TaskNode {
                output: None,
                task: identity.clone(),
                phantom: std::marker::PhantomData,
                // task: Box::new(identity.clone()),
                // dependencies: &(base,),
                dependencies: Box::new((base.clone(),)),
                // "George".to_string(),),
                // dependencies: Box::new(("George",)),
            });

            let dependencies = Box::new((parent1.clone(), parent2.clone()));
            let mut combine = Arc::new(TaskNode {
                output: None,
                task: combine.clone(),
                phantom: std::marker::PhantomData,
                // task: Box::new(combine.clone()),
                // dependencies: &(parent1, parent2),
                dependencies: dependencies.clone(),
            });

            // // sanity check that partial eq works
            // assert_eq!(ScheduleItem(combine.clone()), ScheduleItem(combine.clone()));
            // let lol = Box::new(combine.clone());
            // let lol2 = Box::new(combine.clone());
            // assert_eq!(
            //     ScheduleItem(lol.as_ref().clone()),
            //     ScheduleItem(lol2.as_ref().clone()),
            //     "message"
            // );
            // assert_eq!(
            //     ScheduleItem(combine.clone()),
            //     ScheduleItem(lol.as_ref().clone()),
            //     "message"
            // );

            // assert_eq!(
            //     ScheduleItem(combine.clone()),
            //     ScheduleItem(combine.clone() as Arc<dyn Schedulable>)
            // );

            // assert_eq!(
            //     ScheduleItem(dependencies.as_ref().0.clone()),
            //     ScheduleItem(parent1.clone())
            // );

            // // the to vec is destroying things

            // let hmm: Arc<dyn Schedulable> = combine.dependencies.to_vec()[0].clone();
            // dbg!(format!("{:p}", Arc::as_ptr(&hmm)));
            // dbg!(format!("{:p}", Arc::as_ptr(&parent1)));
            // // dbg!(hmm.ptr.as_ptr());
            // // dbg!(parent1.as_ptr());
            // dbg!(&*hmm as *const _);
            // dbg!(&*parent1 as *const _);
            // dbg!(&*(parent1.clone() as Arc<dyn Schedulable>) as *const _);
            // dbg!(&*(parent1.clone() as Arc<dyn Schedulable>) as *const _ == &*hmm as *const _);
            // dbg!(std::ptr::eq(
            //     &*(parent1.clone() as Arc<dyn Schedulable>) as *const _,
            //     &*hmm as *const _
            // ));
            // dbg!(format!(
            //     "{:p}",
            //     Arc::as_ptr(&(parent1.clone() as Arc<dyn Schedulable>))
            // ));
            // dbg!(Arc::ptr_eq(
            //     &hmm,
            //     &(parent1.clone() as Arc<dyn Schedulable>)
            // ));
            // dbg!(by_address::ByThinAddress(hmm.clone()) == by_address::ByThinAddress(parent1.clone()));

            // assert_eq!(ScheduleItem(hmm.clone()), ScheduleItem(parent1.clone()));

            // // sanity check that hash works
            // use std::hash::Hash;
            // let hasher = std::collections::hash_map::DefaultHasher::new();
            // assert_eq!(
            //     ScheduleItem(combine.clone()).hash(&mut hasher.clone()),
            //     ScheduleItem(combine.clone()).hash(&mut hasher.clone())
            // );
            // assert_eq!(
            //     ScheduleItem(combine.clone()).hash(&mut hasher.clone()),
            //     ScheduleItem(combine.clone() as Arc<dyn Schedulable>).hash(&mut hasher.clone())
            // );

            let mut scheduler = Scheduler::default();
            // scheduler.add_task(base);
            // scheduler.add_task(parent1);
            // scheduler.add_task(parent2);
            scheduler.add_task(combine);

            dbg!(&scheduler);

            render_graph(
                &scheduler,
                &PathBuf::from(file!())
                    .parent()
                    .unwrap()
                    .join("../graphs/basic.svg"),
            )?;

            // write the scheduler loop

            // let mut tasks: FuturesUnordered<Pin<Box<TaskFut>>> = FuturesUnordered::new();
            let mut ready: Vec<Arc<Mutex<dyn Schedulable>>> = scheduler
                .dependencies
                // .iter()
                .keys()
                .filter(|t| t.ready())
                .map(|t| t.0.clone())
                // .cloned()
                // .filter_map(|(t, deps)| {
                //     if deps.is_empty() {
                //         Some(t.0.clone())
                //     } else {
                //         None
                //     }
                // })
                .collect();
            dbg!(&ready);
            // p.ready()).cloned().collect();
            // let mut tasks: FuturesUnordered<askFut>>> = FuturesUnordered::new();
            type TaskFut = dyn Future<Output = Result<Arc<dyn Schedulable>>>;
            let mut tasks: FuturesUnordered<Pin<Box<TaskFut>>> = FuturesUnordered::new();

            loop {
                // check if we are done
                if tasks.is_empty() && ready.is_empty() {
                    break;
                }

                // start running ready tasks
                for p in ready.drain(0..) {
                    tasks.push(Box::pin(async move {
                        p.run().await;
                        Ok(p)
                    }));
                }

                // wait for a task to complete
                match tasks.next().await {
                    Some(Err(err)) => {
                        anyhow::bail!("a task failed: {}", err)
                    }
                    Some(Ok(completed)) => {
                        // update ready tasks
                        let dependants = &scheduler.dependants[&ByThinAddress(completed)];
                        ready.extend(
                            // completed
                            // .dependants
                            dependants
                                // .read()
                                // .unwrap()
                                // .values()
                                .iter()
                                .filter_map(|d| if d.ready() { Some(d.0.clone()) } else { None }), // && !d.published())
                                                                                                   // .cloned(),
                        );
                    }
                    None => {}
                }
            }
            // let deps = Box::new((parent1.clone(), parent2.clone()))
            // find start nodes
            // let mut dependants: HashMap<TaskNode, HashSet<TaskNode>> = HashMap::new();

            // let mut ready: Vec<Arc<Package>> = packages.values().filter(|p| p.ready()).cloned().collect();

            // the arguments
            // let task_fut = node.task.run(node.dependencies);
            // let task_fut = task.run(("George".to_string(),));
            // assert_eq!(task_fut.await, "George");

            // pool.push(Box::pin(async move { task_fut.await }));
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
