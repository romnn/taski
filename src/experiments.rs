mod sealed {
    /*
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
    impl<F, R> Func<()> for F
    where
        F: Fn() -> R,
    {
        type Output = R;
        #[inline]
        fn call(&self, _args: ()) -> Self::Output {
            (*self)()
        }
    }
    impl<T16> HList for Product<T16, ()> {
        type Tuple = (T16,);
        #[inline]
        fn flatten(self) -> Self::Tuple {
            (self.0,)
        }
    }
    impl<T16> Tuple for (T16,) {
        type HList = Product<T16, ()>;
        #[inline]
        fn hlist(self) -> Self::HList {
            Product(self.0, ())
        }
    }

    impl<F, R, T16> Func<Product<T16, ()>> for F
    where
        F: Fn(T16) -> R,
    {
        type Output = R;
        #[inline]
        fn call(&self, args: Product<T16, ()>) -> Self::Output {
            (*self)(args.0)
        }
    }

    impl<F, R, T16> Func<(T16,)> for F
    where
        F: Fn(T16) -> R,
    {
        type Output = R;
        #[inline]
        fn call(&self, args: (T16,)) -> Self::Output {
            (*self)(args.0)
        }
    }
    */

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
}
