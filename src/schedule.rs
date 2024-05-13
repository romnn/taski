use petgraph as pg;

use crate::{
    dag::{self, Dfs, DAG},
    dependency::Dependencies,
    task,
};

use pg::{graph::NodeIndex, Direction::*};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Trait representing a schedulable task node.
///
/// TaskNodes implement this trait.
/// We cannot just use the TaskNode by itself, because we need to combine
/// task nodes with different generic parameters.
#[async_trait::async_trait]
pub trait Schedulable<L> {
    /// Indicates if the schedulable task has succeeded.
    ///
    /// A task is succeeded if its output is available.
    fn succeeded(&self) -> bool;

    /// Fails the schedulable task.
    // fn fail(&self, err: Arc<dyn std::error::Error + Send + Sync + 'static>);
    fn fail(&self, err: Box<dyn std::error::Error + Send + Sync + 'static>);

    /// The result state of the task after completion.
    fn state(&self) -> task::State;

    fn as_argument(&self) -> String;

    fn signature(&self) -> String {
        let arguments: Vec<_> = self
            .dependencies()
            .iter()
            .map(|dep| dep.as_argument())
            .collect();
        format!("{}({})", self.name(), arguments.join(", "))
    }

    /// The creation time of the task.
    fn created_at(&self) -> Instant;

    /// The starting time of the task.
    ///
    /// If the task is still pending, `None` is returned.
    fn started_at(&self) -> Option<Instant>;

    /// The completion time of the task.
    ///
    /// If the task is still pending or running, `None` is returned.
    fn completed_at(&self) -> Option<Instant>;

    /// Unique index of the task node in the DAG graph
    fn index(&self) -> dag::Idx;

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
    fn name(&self) -> &str;

    /// Returns the short name of the schedulable task
    // fn short_name(&self) -> String;

    /// Returns the label of this task.
    fn label(&self) -> &L;

    /// Returns the dependencies of the schedulable task
    fn dependencies(&self) -> Vec<Arc<dyn Schedulable<L>>>;

    /// Indicates if the schedulable task is ready for execution.
    ///
    /// A task is ready if all its dependencies succeeded.
    fn ready(&self) -> bool {
        self.dependencies().iter().all(|d| d.succeeded())
    }

    /// The running_time time of the task.
    ///
    /// If the task has not yet completed, `None` is returned.
    fn running_time(&self) -> Option<Duration> {
        match (self.started_at(), self.completed_at()) {
            (Some(s), Some(e)) => Some(e.duration_since(s)),
            _ => None,
        }
    }

    /// The queue time of the task.
    fn queue_time(&self) -> Duration {
        match self.started_at() {
            Some(s) => s.duration_since(self.created_at()),
            None => self.created_at().elapsed(),
        }
    }
}

impl<L> std::fmt::Debug for dyn Schedulable<L> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
        // write!(f, "{}", self.name())
    }
}

impl<L> std::fmt::Debug for dyn Schedulable<L> + Send + Sync + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
        // write!(f, "{}", self.name())
    }
}

impl<L> std::fmt::Display for dyn Schedulable<L> + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
    }
}

impl<L> std::fmt::Display for dyn Schedulable<L> + Send + Sync + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.signature())
        // write!(f, "{}", self.name())
    }
}

impl<L> Hash for dyn Schedulable<L> + '_ {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index().hash(state);
    }
}

impl<L> Hash for dyn Schedulable<L> + Send + Sync + '_ {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index().hash(state);
    }
}

impl<L> PartialEq for dyn Schedulable<L> + '_ {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(&self.index(), &other.index())
    }
}

impl<L> PartialEq for dyn Schedulable<L> + Send + Sync + '_ {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(&self.index(), &other.index())
    }
}

impl<L> Eq for dyn Schedulable<L> + '_ {}

impl<L> Eq for dyn Schedulable<L> + Send + Sync + '_ {}

/// A task schedule based on a DAG of task nodes.
#[derive(Debug, Clone)]
// pub struct Schedule<'a, L> {
pub struct Schedule<L> {
    pub dag: DAG<task::Ref<L>>,
    // pub dag: DAG<&'a task::Ref<L>>,
    // pub dependency_dag: DAG<task::Ref<L>>,
    // pub dependants_dag: DAG<task::Ref<L>>,
}

impl<L> Default for Schedule<L> {
    // impl<'a, L> Default for Schedule<'a, L> {
    fn default() -> Self {
        Self {
            dag: DAG::default(),
            // dependency_dag: DAG::default(),
            // dependants_dag: DAG::default(),
        }
    }
}

// impl<L> std::fmt::Debug for Schedule<L> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("Schedule")
//             .field("dependencies", &self.dependencies)
//             .field("dependants", &self.dependants)
//             .finish()
//     }
// }

impl<L> Schedule<L> {
    /// Add a new task to the graph.
    ///
    /// Dependencies for the task must be references to tasks that have
    /// already been added to the task graph (Arc<TaskNode>)
    pub fn add_node<I, O, T, D>(
        &mut self,
        // task: impl Into<T>,
        task: T,
        deps: D,
        label: L,
    ) -> Arc<task::Node<I, O, L>>
    where
        T: task::Task<I, O> + Send + Sync + 'static,
        // T: task::Task<I, O> + std::fmt::Debug + Send + Sync + 'static,
        D: Dependencies<I, L> + Send + Sync + 'static,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Sync + 'static,
    {
        let index = dag::Idx::new(self.dag.node_count());
        // let task = task.into();
        let node = Arc::new(task::Node::new(task, deps, label, index));
        // let node = Arc::new(task::Node {
        //     task_name: task.name(),
        //     label,
        //     created_at: Instant::now(),
        //     // started_at: RwLock::new(None),
        //     // completed_at: RwLock::new(None),
        //     // state: RwLock::new(task::InternalState::Pending(Box::new(task))),
        //     inner: RwLock::new(task::NodeInner::new(task)),
        //     dependencies: Box::new(deps),
        //     index,
        // });

        // let my_node = pg::graph::Node::de {
        //     node.clone(),
        //     next: [pg::graph::EdgeIndex::end(), pg::graph::EdgeIndex::end()],
        // };
        // let node_idx = NodeIndex::new(self.nodes.len());
        // // check for max capacity, except if we use usize
        // assert!(<Ix as IndexType>::max().index() == !0 || NodeIndex::end() != node_idx);
        // self.nodes.push(node);

        // TODO: return the index instead of the node here?
        // that way we always have to query the schedule for
        // introspection, but that could be fine as long as
        // ownership allows, but i suppose it could be bad for
        // losing type information

        // add node
        let node_index = self.dag.add_node(node.clone());
        assert_eq!(node_index, index);
        // let node = self.dag.node_weight_mut(index).unwrap();
        // Arc::get_mut(node).unwrap().index = idx;

        // add edges to dependencies
        for dep in node.dependencies() {
            self.dag.add_edge(dep.index().into(), node_index, ());
            // todo: get or insert dep here
            // let dep_idx = self.dag.node_weight(dep);

            // self.dependants_dag.add_node(node.clone());
            // self.dependency_dag.insert(dep.clone());
        }

        // // check for circles here
        // let mut seen: HashSet<task::Ref<L>> = HashSet::new();
        // let mut stack: Vec<task::Ref<L>> = vec![node.clone()];
        //
        // while let Some(node) = stack.pop() {
        //     if !seen.insert(node.clone()) {
        //         continue;
        //     }
        //     let dependencies = self.dependency_dag.entry(node.clone()).or_default();
        //
        //     for dep in node.dependencies() {
        //         let dependants = self.dependants_dag.entry(dep.clone()).or_default();
        //         dependants.insert(node.clone());
        //         dependencies.insert(dep.clone());
        //         stack.push(dep);
        //     }
        // }

        node
    }

    pub fn add_closure<C, F, I, O, D>(
        &mut self,
        closure: C,
        deps: D,
        label: L,
    ) -> Arc<task::Node<I, O, L>>
    where
        // T: task::BoxedClosure<I, O> + Send + Sync + 'static,
        // C: task::UserClosure<F, I, O> + Send + Sync + 'static,
        C: task::UserClosure<F, I, O> + Send + Sync + 'static,
        // F: futures::Future<Output = task::Result<O>>,
        F: futures::Future<Output = task::Result<O>> + Send + Sync + 'static,

        // T: task::Task<I, O> + std::fmt::Debug + Send + Sync + 'static,
        D: Dependencies<I, L> + Send + Sync + 'static,
        I: std::fmt::Debug + Send + Sync + 'static,
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Sync + 'static,
    {
        let index = dag::Idx::new(self.dag.node_count());
        let node = Arc::new(task::Node::closure(closure, deps, label, index));
        let node_index = self.dag.add_node(node.clone());
        assert_eq!(node_index, index);
        for dep in node.dependencies() {
            self.dag.add_edge(dep.index().into(), node_index, ());
        }
        node
    }

    pub fn add_input<O>(&mut self, input: O, label: L) -> Arc<task::Node<(), O, L>>
    where
        O: std::fmt::Debug + Send + Sync + 'static,
        L: std::fmt::Debug + Sync + 'static,
    {
        self.add_node(task::Input::from(input), (), label)
    }

    /// Marks all dependants as failed.
    ///
    /// Optionally also marks all dependencies as failed if they have no pending dependants.
    ///
    /// TODO: is this correct and sufficient?
    /// TODO: really test this...
    // pub fn fail_dependants<'a>(&'a mut self, root: &'a task::Ref<L>, dependencies: bool) {
    pub fn fail_dependants<'a>(&'a mut self, root: dag::Idx, dependencies: bool) {
        let mut queue = vec![root];
        let mut processed = HashSet::new();
        processed.insert(root);

        while let Some(idx) = queue.pop() {
            dbg!(&queue);
            dbg!(&processed);

            // fail all dependants
            let mut new_processed = vec![];

            let dependants = pg::visit::Reversed(&self.dag);

            // // check if we can get the neighbors
            // use pg::visit::IntoNeighborsDirected;
            //
            // self.dag
            //     .neighbors_directed(task.index().into(), Incoming);
            // dependants.neighbors_directed(task.index().into(), Incoming);

            // let dfs = Dfs::new(&dependants, task);
            // let dfs = Dfs::new(&dependants, task.index().into());
            let dfs = Dfs::new(&dependants, idx);
            // let dfs = dfs.filter(|dep: &task::Ref<L>| {
            let mut dfs = dfs.filter(|dep_idx: &NodeIndex<usize>| {
                let dep = &self.dag[*dep_idx];
                dep.state().is_pending() && !processed.contains(dep_idx)
            });

            while let Some(dep_idx) = dfs.next(&self.dag) {
                // todo: link to failed dependency
                // if processed.insert(dependant) {
                let dependant = &self.dag[dep_idx];
                // new_processed.push(dependant);

                dependant.fail(Box::new(task::Error::FailedDependency));
                // processed.insert(dependant);

                new_processed.push(dep_idx);
                queue.push(dep_idx);

                //
            }

            // let dependants = self
            //     .dependants_dag
            //     .traverse(task, None, |dep: &&task::Ref<L>| {
            //         matches!(dep.state(), task::CompletionResult::Pending)
            //             && !processed.contains(dep)
            //     });
            // for (_, dependant) in dependants {
            //     // todo: link to failed dependency
            //     // if processed.insert(dependant) {
            //     new_processed.push(dependant);
            //     dependant.fail(Arc::new(task::Error::FailedDependency));
            //     // processed.insert(dependant);
            //     queue.push(dependant);
            //     // }
            // }
            // fail all dependencies of task
            if dependencies {
                // fail all dependencies
                todo!();
                // let dependencies =
                //     self.dependency_dag
                //         .traverse(task, None, |dep: &&task::Ref<L>| {
                //             matches!(dep.state(), task::CompletionResult::Pending)
                //                 && !processed.contains(task)
                //         });
                // for (_, dependency) in dependencies {
                //     // fail dependency
                //     // if processed.insert(dependency) {
                //     dependency.fail(Arc::new(task::Error::FailedDependency));
                //     // processed.insert(dependency);
                //     queue.push(dependency);
                //     new_processed.push(dependency);
                //     // }
                // }
            }

            processed.extend(new_processed);
        }
    }

    // pub fn ready(&self) -> impl Iterator<Item = NodeIndex<usize>> {
    pub fn ready(&self) -> impl Iterator<Item = dag::Idx> + '_ {
        self.dag.node_indices().filter(|idx| self.dag[*idx].ready())
    }

    pub fn running(&self) -> impl Iterator<Item = dag::Idx> + '_ {
        self.dag.node_indices().filter(|idx| self.dag[*idx].ready())
    }

    // pub fn done(&self) -> bool {
    //     // schedule is
    //     self.ready().next().is_none()
    //     // use task::CompletionResult::*;
    //     // self.dag
    //     //     .node_indices()
    //     //     .all(|idx| matches!(self.dag[idx].state(), Failed | Succeeded))
    // }

    // /// Iterator over the immediate dependencies of a task
    // pub fn dependencies<'a>(
    //     &'a self,
    //     task: &'a task::Ref<L>,
    // // ) -> Dfs<'a, task::Ref<L>, impl Fn(&&task::Ref<L>) -> bool> {
    // ) -> pg::graph::Neighbors<'_, (), usize> {
    //     self.dag.neighbors_directed(task.index().into(), Incoming)
    //     // self.dependency_dag.traverse(task, Some(1), |_| true)
    // }
    //
    // /// Iterator over the immediate dependants of a task
    // pub fn dependants<'a>(
    //     &'a self,
    //     task: &'a task::Ref<L>,
    // // ) -> Dfs<'a, task::Ref<L>, impl Fn(&&task::Ref<L>) -> bool> {
    // ) -> pg::graph::Neighbors<'_, (), usize> {
    //     self.dag.neighbors_directed(task.index().into(), Outgoing)
    //     // self.dependants_dag.traverse(task, Some(1), |_| true)
    // }

    // /// Iterator over all recursive dependencies of a task
    // pub fn rec_dependencies<'a>(
    //     &'a self,
    //     task: &'a task::Ref<L>,
    // ) -> Dfs<'a, task::Ref<L>, impl Fn(&&task::Ref<L>) -> bool> {
    //     self.dependency_dag.traverse(task, None, |_| true)
    // }
    //
    // /// Iterator over all recursive dependants of a task
    // pub fn rec_dependants<'a>(
    //     &'a self,
    //     task: &'a task::Ref<L>,
    // ) -> Dfs<'a, task::Ref<L>, impl Fn(&&task::Ref<L>) -> bool> {
    //     self.dependants_dag.traverse(task, None, |_| true)
    // }
}

#[cfg(feature = "render")]
pub mod render {
    use crate::{schedule::Schedulable, task};

    use layout::{
        backends::svg::SVGWriter,
        core::{self, base::Orientation, color::Color, style},
        std_shapes::shapes,
        topo::layout::VisualGraph,
    };
    use petgraph as pg;
    use pg::Direction::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    impl<L> super::Schedule<L> {
        /// Render the task graph as an svg image.
        ///
        /// # Errors
        /// - If writing to the specified output path fails.
        pub fn render_to(&self, path: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(path.as_ref())?;
            let mut writer = std::io::BufWriter::new(file);
            self.render_to_writer(&mut writer)
        }

        /// Render the task graph as an svg image.
        ///
        /// # Errors
        /// - If writing to the specified output path fails.
        pub fn render_to_writer(
            &self,
            mut writer: impl std::io::Write,
        ) -> Result<(), std::io::Error> {
            let content = self.render();
            writer.write_all(content.as_bytes())?;
            Ok(())
        }

        /// Render the task graph as an svg image.
        #[must_use]
        pub fn render(&self) -> String {
            fn node<LL>(node: &task::Ref<LL>) -> shapes::Element {
                let node_style = style::StyleAttr {
                    line_color: Color::new(0x0000_00FF),
                    line_width: 2,
                    fill_color: Some(Color::new(0xB4B3_B2FF)),
                    rounded: 0,
                    font_size: 15,
                };
                let size = core::geometry::Point { x: 100.0, y: 100.0 };
                shapes::Element::create(
                    shapes::ShapeKind::Circle(format!("{node}")),
                    node_style,
                    Orientation::TopToBottom,
                    size,
                )
            }

            let mut graph = VisualGraph::new(Orientation::TopToBottom);

            let mut handles: HashMap<Arc<dyn Schedulable<L>>, layout::adt::dag::NodeHandle> =
                HashMap::new();

            // for (task, deps) in &self.dependency_dag {
            for idx in self.dag.node_indices() {
                let task = &self.dag[idx];
                let deps = self
                    .dag
                    .neighbors_directed(idx, Incoming)
                    .map(|idx| &self.dag[idx]);

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
            backend.finalize()
        }
    }
}
