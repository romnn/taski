use color_eyre::eyre;

use crate::{CombineAudio, Download, Options};

use std::sync::Arc;
use std::time::Instant;
use taski::trace;
use tempfile::TempDir;

async fn trace_task<T, I, O>(
    trace: &mut trace::Trace<usize>,
    task: Box<T>,
    args: I,
) -> Result<O, taski::Error>
where
    T: taski::task::Task<I, O>,
{
    let label = task.name().clone();
    let color = task.color();
    let start = Instant::now();
    let result = taski::task::Task::run(task, args)
        .await
        .map_err(taski::Error)?;
    let end = Instant::now();
    let traced = taski::trace::Task {
        label,
        color,
        start,
        end,
    };
    trace.tasks.push((trace.tasks.len(), traced));
    Ok(result)
}

pub async fn run(options: Options) -> eyre::Result<()> {
    let start = Instant::now();
    let tmp_dir = Arc::new(TempDir::new()?);

    let Options {
        first_url,
        second_url,
        ..
    } = options;

    let download = Box::new(Download {
        tmp_dir: tmp_dir.clone(),
        client: reqwest::Client::new(),
    });

    let combine = Box::new(CombineAudio {
        tmp_dir: tmp_dir.clone(),
    });

    let mut trace = trace::Trace::new();
    let d1 = trace_task(
        &mut trace,
        download.clone(),
        ((first_url, "audio1.mp3".to_string()),),
    )
    .await?;

    let d2 = trace_task(
        &mut trace,
        download.clone(),
        ((second_url, "audio2.mp3".to_string()),),
    )
    .await?;

    let _result = trace_task(&mut trace, combine, (d1, d2)).await?;

    if super::should_render() {
        let path = super::manifest_dir().join("serial_trace.svg");
        println!("rendering trace to {}", path.display());
        trace.render_to(path)?;
    }

    println!("completed in {:.2?}", start.elapsed());
    Ok(())
}
