use color_eyre::eyre;

mod async_dag;
mod taski;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let async_dag_result = async_dag::run().await;
    let taski_result = taski::run().await;
    assert_eq!(async_dag_result, Some(7));
    assert_eq!(taski_result, Some(7));
    Ok(())
}
