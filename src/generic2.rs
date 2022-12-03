mod warp {
    use async_trait::async_trait;
    use futures::future::BoxFuture;
    use futures::stream::{FuturesUnordered, StreamExt};
    use std::future::Future;
    use std::pin::Pin;

    #[derive(Debug)]
    pub struct Product<H, T: IntoTuple>(pub(crate) H, pub(crate) T);

    // Converts Product (and ()) into tuples.
    pub trait IntoTuple: Sized {
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

    #[async_trait]
    pub trait Dependency<O> {
    }

    // call dependencies with arguments expanded
    
    #[async_trait]
    pub trait Task<I, O> {
        // do not return a future, make this function the future itself?
        // async ...
        // fn run(&mut self, input: I) -> Pin<Box<dyn Future<Output = O> + Send + Sync>>;
        async fn run(&mut self, input: I) -> O;
        // fn dependencies(&self) -> Vec<Dependency<I>; // vec
    }
    
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

    pub struct Scheduler {
        // Future<Output = PoolResult<(I, Result<O, E>)>> + Send + Sync
        // pub pool: FuturesUnordered<Pin<Box<dyn Task>>>,
    }

    // impl Scheduler {
    //     fn new()
    // }

    #[cfg(test)]
    mod tests {
        use super::*;
        use anyhow::Result;
        use async_trait::async_trait;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn test_basic_scheduler() -> Result<()> {
            // let mut sched = Scheduler
            // type Pool = FuturesUnordered<Pin<Box<dyn InFlightTaskFutTask>>>;
            // type Pool = FuturesUnordered<InFlightTaskFutTask>;

            // let mut pool: Pool = FuturesUnordered::new();

            struct SimpleTask {
                name: String,
            }

            #[async_trait]
            impl Task<(String,), u32> for SimpleTask {
                async fn run(&mut self, input: (String,)) -> u32 {
                    42
                }

                // fn dependencies(&self) -> () {
                //     ()
                // }
            }

            let mut task = SimpleTask {
                name: "Roman".to_string(),
            };

            // the arguments
            let task_fut = task.run(("George".to_string(),));

            // pool.push(Box::pin(async move { task_fut.await }));
            Ok(())
        }
    }

    generics! {
        T1
        // T2,
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
