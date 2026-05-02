use signex_library_server::router_with_in_memory_state;

/// Default bind address — loopback only. M1: previously `0.0.0.0:3535`,
/// which exposes the service on every interface and pairs poorly with the
/// (until v0.9) unauthenticated handlers. Override via `SIGNEX_LIBRARY_BIND`
/// for deployments that need an explicit interface (`0.0.0.0:3535` in
/// container images, etc.).
const DEFAULT_BIND: &str = "127.0.0.1:3535";
const BIND_ENV: &str = "SIGNEX_LIBRARY_BIND";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let bind = std::env::var(BIND_ENV).unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(
        "signex-library-server listening on {}",
        listener.local_addr()?
    );
    // CORS is intentionally not configured here — the loopback bind keeps
    // browser callers out by default. v0.9.2 will add an explicit
    // `CorsLayer` once the front-end origin contract is finalised. (M1)
    let app = router_with_in_memory_state().await?;
    axum::serve(listener, app).await?;
    Ok(())
}
