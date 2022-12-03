/// In v1, just try to express that the n=2 array in dependencies is the argument for task
mod sealed {
    
    #[derive(Debug)]
    // head and tail ?
    struct Product<H, T: HList>(pub H, pub T);

    /// HList allows a Product<String, Product<u32, Product<bool, ()>>>
    /// to be converted into a (String, u32, bool)
    trait HList: Sized {
        type Tuple: Tuple<HList = Self>;

        fn flatten(self) -> Self::Tuple;
    }

    /// Tuple allows a tuple (String, u32, bool) to be converted into a
    /// Product<String, Product<u32, Product<bool, ()>>>
    trait Tuple {
        type HList: HList<Tuple = Self>;

        fn hlist(self) -> Self::HList;

        #[inline]
        fn combine<T>(self, other: T) -> CombinedTuples<Self, T>
        where
            Self: Sized,
            T: Tuple,
            Self::HList: Combine<T::HList>,
        {
            let new_product = self.hlist().combine(other.hlist());
            new_product.flatten()
        }
    }

    pub type CombinedTuples<T, U> =
        <<<T as Tuple>::HList as Combine<<U as Tuple>::HList>>::Output as HList>::Tuple;
    
    // Combine allows two products to be combined
    pub trait Combine<T: HList> {
        type Output: HList;

        fn combine(self, other: T) -> Self::Output;
    }

    /// base case for recursive combine empty unit ()
    /// does not include the () in the final tuple
    impl<T: HList> Combine<T> for () {
        type Output = T;
        #[inline]
        fn combine(self, other: T) -> Self::Output {
            other
        }
    }

    /// head H, tail and U that wil be 
    impl<H, T: HList, U: HList> Combine<U> for Product<H, T>
    where
        T: Combine<U>,
        Product<H, <T as Combine<U>>::Output>: HList,
    {
        type Output = Product<H, <T as Combine<U>>::Output>;

        #[inline]
        fn combine(self, other: U) -> Self::Output {
            Product(self.0, self.1.combine(other))
        }
    }

    impl HList for () {
        type Tuple = ();

        #[inline]
        fn flatten(self) -> Self::Tuple {}
    }

    impl Tuple for () {
        type HList = ();

        #[inline]
        fn hlist(self) -> Self::HList {}
    }

    // why is this hard?
    // all funcs can have different inputs which we do not want to quantify
    // Func1 -> A, Func2 -> B, Func3 -> C => HList<A, B, C>

    /// task node with dependencies
    pub struct TaskNode<I> where I: HList {
        pub task: I,
        // dependencies should cast to (C1, C2, C2) = C
        pub dependencies: I,
        // pub dependencies: Vec<Box<dyn IntoTask<I, C, O, E>>>,
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // lets say we have adder for integers
        // the dependencies are just identities that return a value

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn test_dependencies() -> Result<()> {
            // TaskNode {
            //     task: Task {
            //         id: self.id,
            //         task: Box::new(move |ctx, prereqs| {
            //             Box::pin(async move {
            //                 crate::debug!(id);
            //                 crate::debug!(ctx);
            //                 crate::debug!(prereqs);
            //                 sleep(Duration::from_secs(2)).await;
            //                 Ok(id)
            //             })
            //         }),
            //     },
            //     dependencies: self.dependencies,
            // }
        }
    }
}
