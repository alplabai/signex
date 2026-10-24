use std::net::SocketAddr;

use signex_library_server::{
    API_TOKEN_ENV, AppState, DATABASE_URL_ENV, router_with_in_memory_state, router_with_state,
    with_rate_limit,
};

/// Default bind address — loopback only. Previously `0.0.0.0:3535`, which
/// exposes the service on every interface. Override via `SIGNEX_LIBRARY_BIND`
/// for deployments that need an explicit interface (`0.0.0.0:3535` in
/// container images, etc.).
const DEFAULT_BIND: &str = "127.0.0.1:3535";
const BIND_ENV: &str = "SIGNEX_LIBRARY_BIND";

/// Parse a `host:port` bind string and return whether the host is a
/// loopback (127.0.0.0/8 or [::1]). IPv6 zone-id addresses (`fe80::1%lo0`)
/// are treated as non-loopback even when they happen to resolve to a
/// loopback interface; we want the strictest possible guard.
fn is_loopback_bind(bind: &str) -> bool {
    use std::net::SocketAddr;
    bind.parse::<SocketAddr>()
        .map(|sa| sa.ip().is_loopback())
        .unwrap_or(false)
}

/// HI-1 + persistence policy: a non-loopback bind requires both an API token
/// and a persistent database URL. Loopback dev mode needs neither.
fn check_bind_policy(bind: &str, token_set: bool, db_url_set: bool) -> anyhow::Result<()> {
    if is_loopback_bind(bind) {
        return Ok(());
    }
    if !token_set {
        anyhow::bail!(
            "refusing to bind to non-loopback address `{bind}` without `{API_TOKEN_ENV}` set;              set the env var or bind to 127.0.0.1 / [::1]"
        );
    }
    if !db_url_set {
        anyhow::bail!(
            "refusing to bind to non-loopback address `{bind}` with ephemeral in-memory storage;              set `{DATABASE_URL_ENV}` to a persistent database (postgres:// or sqlite://<file>)"
        );
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let bind = std::env::var(BIND_ENV).unwrap_or_else(|_| DEFAULT_BIND.to_string());

    // HI-1: refuse to start unauthenticated on a non-loopback bind.
    // Loopback dev mode without a token is fine; binding 0.0.0.0 / a
    // routable interface without a token would expose every CRUD route
    // to the network with only a `tracing::warn!`, so we exit instead.
    let token_set = std::env::var(API_TOKEN_ENV)
        .map(|t| !t.is_empty())
        .unwrap_or(false);

    // Persistent storage. Without `SIGNEX_DATABASE_URL` the server falls
    // back to an ephemeral in-memory SQLite that loses every row on
    // restart — fine for loopback dev, catastrophic in production. A
    // non-loopback (public) bind therefore REQUIRES a database URL.
    let db_url = std::env::var(DATABASE_URL_ENV)
        .ok()
        .filter(|s| !s.is_empty());
    check_bind_policy(&bind, token_set, db_url.is_some())?;

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(
        "signex-library-server listening on {}",
        listener.local_addr()?
    );

    let router = match &db_url {
        Some(url) => {
            let state = AppState::connect(url).await?;
            state.migrate().await?;
            tracing::info!("persistent database backend connected");
            router_with_state(state)
        }
        None => {
            tracing::warn!(
                "no {DATABASE_URL_ENV} set — using EPHEMERAL in-memory SQLite (loopback dev mode); \
                 ALL DATA IS LOST ON RESTART"
            );
            router_with_in_memory_state().await?
        }
    };

    // HI-2: per-IP rate limit applied here (not in `router_with_state`)
    // so integration tests can hit the bare router via `oneshot` without
    // going through the governor's `ConnectInfo` extractor.
    let app = with_rate_limit(router);
    // `into_make_service_with_connect_info::<SocketAddr>()` plumbs the
    // peer IP into request extensions — `tower_governor` reads it from
    // there to bucket requests by source.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_bind_policy, is_loopback_bind};

    #[test]
    fn loopback_binds_are_accepted() {
        for b in ["127.0.0.1:3535", "127.5.5.5:80", "[::1]:3535"] {
            assert!(is_loopback_bind(b), "{b} should be loopback");
        }
    }

    #[test]
    fn non_loopback_or_unparseable_binds_are_rejected() {
        for b in [
            "0.0.0.0:3535",
            "[::]:3535",
            "192.168.1.10:3535",
            "[fe80::1%lo0]:3535",
            "localhost:3535",
            "127.0.0.1",
            "",
        ] {
            assert!(!is_loopback_bind(b), "{b:?} must not count as loopback");
        }
    }

    #[test]
    fn bind_policy_covers_all_combinations() {
        let public = "0.0.0.0:3535";
        let loopback = "127.0.0.1:3535";
        // loopback: always allowed
        for (t, d) in [(false, false), (true, false), (false, true), (true, true)] {
            assert!(check_bind_policy(loopback, t, d).is_ok());
        }
        // non-loopback: needs token AND db url
        let no_token = check_bind_policy(public, false, true)
            .unwrap_err()
            .to_string();
        assert!(no_token.contains("without"), "{no_token}");
        let no_db = check_bind_policy(public, true, false)
            .unwrap_err()
            .to_string();
        assert!(no_db.contains("ephemeral"), "{no_db}");
        assert!(check_bind_policy(public, false, false).is_err());
        assert!(check_bind_policy(public, true, true).is_ok());
    }
}
