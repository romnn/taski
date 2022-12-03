/// task has id I, input C, outpt O, error E

// pub type TaskFun<C, I, O, E> = Box<
//     dyn FnOnce(C, HashMap<I, O>) -> Pin<Box<dyn Future<Output = Result<O, E>> + Send + Sync>>
//         + Send
//         + Sync,
// >;


// /// a task
// pub struct Task<I, C, O, E> {
//     /// unique identifier for this task
//     pub id: I,
//     /// task function
//     // pub task: TaskFun<C, I, O, E>,
// }

// /// task node with dependencies
// pub struct TaskNode<I, C, O, E> {
//     pub task: Task<I, C, O, E>,
//     // dependencies should cast to (C1, C2, C2) = C
//     pub dependencies: Vec<Box<dyn IntoTask<I, C, O, E>>>,
// }

/// task node with dependencies
pub struct TaskNode<I, O> {
    pub task: Task<I, O>,
    // dependencies should cast to (C1, C2, C2) = C
    pub dependencies: Vec<Box<dyn IntoTask<I, C, O, E>>>,
}


#[cfg(test)]
mod tests {
    use super::*;
    
    // lets say we have adder for integers
    // the dependencies are just identities that return a value

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_dependencies() -> Result<()> {
        TaskNode {
            task: Task {
                id: self.id,
                task: Box::new(move |ctx, prereqs| {
                    Box::pin(async move {
                        crate::debug!(id);
                        crate::debug!(ctx);
                        crate::debug!(prereqs);
                        sleep(Duration::from_secs(2)).await;
                        Ok(id)
                    })
                }),
            },
            dependencies: self.dependencies,
        }
    }
}
