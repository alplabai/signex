use signex_library_server::router;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3535").await?;
    tracing::info!(
        "signex-library-server listening on {}",
        listener.local_addr()?
    );
    axum::serve(listener, router()).await?;
    Ok(())
}
