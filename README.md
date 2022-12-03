## djtool task scheduler

#### Considerations
- do we need products? why? can we get away with only tuples
    - i guess so, they allow single argument for use in traits where we would need a new trait for each number of args otherwise
- how can we express the invoke trait?
- use a task trait rather than a task struct, users will want to pass their own context and so on and have their own methods ...
    - only drawback: can only call the async runner function with the Product type (single argument)
        - possible solution: define for the product type
        - use macros to expand for different traits

```bash
cargo expand generic2
```

#### TODO
- use the warp generics, but name them better
- replace the function


_Note_: If this implementation turns out to be well designed and useful for different situations, it will become a stand alone external dependency for djtool.

#### Goals of this implementation

There exist many different task scheduler implementations, the goals of this implementation are as follows:

- support async tasks
- support directed acyclic graphs with cycle detection
- allow custom scheduling policies based on a custom ID type, which could enforce complex constraints by using labels
- allow concurrent adding of tasks while the scheduler is executing, so that tasks can be streamed into the scheduler

#### Usage

tba
