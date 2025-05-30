## taski

[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/romnn/taski/build.yaml?branch=main&label=build">](https://github.com/romnn/taski/actions/workflows/build.yaml)
[<img alt="test status" src="https://img.shields.io/github/actions/workflow/status/romnn/taski/test.yaml?branch=main&label=test">](https://github.com/romnn/taski/actions/workflows/test.yaml)
[![dependency status](https://deps.rs/repo/github/romnn/taski/status.svg)](https://deps.rs/repo/github/romnn/taski)
[<img alt="crates.io" src="https://img.shields.io/crates/v/taski">](https://crates.io/crates/taski)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/taski/latest?label=docs.rs">](https://docs.rs/taski)

```bash
cargo add taski
```

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

#### Development

```bash
cargo install cargo-expand
cargo expand ::task
```

There exist many different task scheduler implementations, the goals of this implementation are as follows:

- support async tasks
- support directed acyclic graphs with cycle detection
- allow custom scheduling policies based on a custom ID type, which could enforce complex constraints by using labels
- allow concurrent adding of tasks while the scheduler is executing, so that tasks can be streamed into the scheduler
