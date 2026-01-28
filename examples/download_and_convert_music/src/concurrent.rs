use crate::{CombineAudio, Download, Label, Options};

use color_eyre::eyre;
use std::sync::Arc;
use std::time::Instant;
use taski::PolicyExecutor;
use tempfile::TempDir;

pub async fn run(options: Options) -> eyre::Result<()> {
    let start = Instant::now();
    let tmp_dir = Arc::new(TempDir::new()?);

    let Options {
        first_url,
        second_url,
        output_path,
    } = options;

    let download = Download {
        tmp_dir: tmp_dir.clone(),
        client: reqwest::Client::new(),
    };

    let combine = CombineAudio {
        tmp_dir: tmp_dir.clone(),
    };

    taski::make_guard!(guard);
    let mut graph = taski::Schedule::new(guard);
    let i1 = graph.add_input((first_url, "audio1.mp3".to_string()), Label::Input);
    let i2 = graph.add_input((second_url, "audio2.mp3".to_string()), Label::Input);

    let d1 = graph.add_node(download.clone(), (i1,), Label::Download);
    let d2 = graph.add_node(download.clone(), (i2,), Label::Download);

    let result = graph.add_node(combine, (d1, d2), Label::Combine);

    let mut executor = PolicyExecutor::fifo(graph);

    // run all tasks
    executor.run().await;

    if super::should_render() {
        let path = super::manifest_dir().join("concurrent_graph.svg");
        println!("rendering graph to {}", path.display());
        executor.schedule.render_to(path)?;

        let path = super::manifest_dir().join("concurrent_trace.svg");
        println!("rendering trace to {}", path.display());
        executor.trace.render_to(path)?;
    }

    // copy to example dir so we can test
    if let Some(path) = output_path {
        if let Some(output_path) = executor.execution.output_ref(result).cloned() {
            tokio::fs::copy(output_path, path).await?;
        }
    }

    println!("completed in {:.2?}", start.elapsed());
    Ok(())
}
