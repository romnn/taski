#![allow(clippy::missing_panics_doc)]

use color_eyre::eyre;

mod async_dag;
mod taski_closures;
mod taski_tasks;

use std::path::PathBuf;

#[must_use]
pub fn manifest_dir() -> PathBuf {
    PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
}

#[must_use]
pub fn should_render() -> bool {
    std::env::var("RENDER")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
        == "yes"
}

// Render the DAG graph and an execution trace.
//
// NOTE: this requires the "render" feature of taski.
pub fn render<P, L: 'static>(
    executor: &taski::PolicyExecutor<P, L>,
    suffix: &str,
) -> eyre::Result<()> {
    if !should_render() {
        return Ok(());
    }
    executor
        .schedule()
        .render_to(manifest_dir().join(format!("taski_{suffix}_graph.svg")))?;
    executor
        .trace()
        .render_to(manifest_dir().join(format!("taski_{suffix}_trace.svg")))?;
    Ok(())
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let async_dag_result = async_dag::run().await;
    let taski_result = taski_tasks::run().await?;
    let taski_closures_result = taski_closures::run().await?;
    assert_eq!(async_dag_result, Some(7));
    assert_eq!(taski_result, Some(7));
    assert_eq!(taski_closures_result, Some(7));
    Ok(())
}
