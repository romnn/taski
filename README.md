## taski

[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/romnn/taski/build.yml?branch=main&label=build">](https://github.com/romnn/taski/actions/workflows/build.yml)
[<img alt="test status" src="https://img.shields.io/github/actions/workflow/status/romnn/taski/test.yml?branch=main&label=test">](https://github.com/romnn/taski/actions/workflows/test.yml)
[<img alt="benchmarks" src="https://img.shields.io/github/actions/workflow/status/romnn/taski/bench.yml?branch=main&label=bench">](https://romnn.github.io/taski/)
[<img alt="crates.io" src="https://img.shields.io/crates/v/taski">](https://crates.io/crates/taski)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/taski/latest?label=docs.rs">](https://docs.rs/taski)

#### Design decisions

- task node is internal, because this is where the state is kept (interior mutability) and where the unique index is assigned to
  - construction from the user not possible
  - only possible to creatge using `add_task`
  - only valid input as dependency (we know it was added (otherwise we do)) and we know it cannot have circles
  - user cannot run any methods on it? wrong because of task trait
- use a result type, to allow for fail fast strategies
  - user workaround: use infallible and propagate an Option<O>
- clone outputs as inputs, this is more efficient for small data such as i32
  - for large data (or data that cannot be cloned), just return an Arc<O>

#### TODO (generic3)

- add a builder for the scheduler
- add a schedule trait

#### DONE (needs testing)

- implement fail fast using some graph traversal magic

#### DONE (generic3)

- add an executor trait that can execute a schedule
- add a policy trait
- implement custom arbiter that can access labels of nodes
  - we want labels
  - we want current running tasks with start time etc.
- arc clone task outputs (we wont do that)
- return results with boxed? errors

#### Considerations

- do we need products? why? can we get away with only tuples
  - i guess so, they allow single argument for use in traits where we would need a new trait for each number of args otherwise
- how can we express the invoke trait?
- use a task trait rather than a task struct, users will want to pass their own context and so on and have their own methods ...
  - only drawback: can only call the async runner function with the Product type (single argument)
    - possible solution: define for the product type
    - use macros to expand for different traits

```bash
cargo install cargo-expand
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
