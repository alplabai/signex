use std::net::SocketAddr;

use signex_library_server::{
    API_TOKEN_ENV, AppState, DATABASE_URL_ENV, router_with_in_memory_state, router_with_state,
    with_rate_limit_trusted,
};

/// Default bind address — loopback only. Previously `0.0.0.0:3535`, which
/// exposes the service on every interface. Override via `SIGNEX_LIBRARY_BIND`
/// for deployments that need an explicit interface (`0.0.0.0:3535` in
/// container images, etc.).
const DEFAULT_BIND: &str = "127.0.0.1:3535";
const BIND_ENV: &str = "SIGNEX_LIBRARY_BIND";
/// Explicit opt-in to serve plaintext HTTP on a non-loopback address.
const INSECURE_BIND_ENV: &str = "SIGNEX_ALLOW_INSECURE_BIND";
const INSECURE_BIND_FLAG: &str = "--allow-insecure-bind";
/// Comma-separated proxy IPs whose `X-Forwarded-For` / `Forwarded` is believed
/// by the rate limiter.
const TRUSTED_PROXIES_ENV: &str = "SIGNEX_TRUSTED_PROXIES";
/// `x-forwarded-for` | `forwarded`: the one header the trusted proxy writes.
/// Required when `SIGNEX_TRUSTED_PROXIES` is non-empty; the other is ignored.
const TRUSTED_PROXY_HEADER_ENV: &str = "SIGNEX_TRUSTED_PROXY_HEADER";

/// `1` / `true` / `yes` (any case) enable the opt-in; any other set value is
/// warned about and treated as off.
fn insecure_env_enabled(raw: Option<&str>) -> bool {
    let Some(v) = raw else { return false };
    match v.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => true,
        "" | "0" | "false" | "no" => false,
        other => {
            tracing::warn!(
                "{INSECURE_BIND_ENV}=`{other}` not recognised (use 1/true/yes); ignored"
            );
            false
        }
    }
}

/// Header setting is mandatory exactly when proxies are trusted.
fn parse_proxy_header(
    proxies: &[std::net::IpAddr],
    raw: Option<&str>,
) -> anyhow::Result<signex_library_server::ProxyHeader> {
    let raw = raw.map(str::trim).filter(|s| !s.is_empty());
    match (proxies.is_empty(), raw) {
        (true, None) => Ok(Default::default()),
        (false, None) => anyhow::bail!(
            "{TRUSTED_PROXY_HEADER_ENV} must be `x-forwarded-for` or `forwarded` when \
             {TRUSTED_PROXIES_ENV} is set"
        ),
        (_, Some(r)) => r
            .parse()
            .map_err(|e| anyhow::anyhow!("{TRUSTED_PROXY_HEADER_ENV}: {e}")),
    }
}

/// Parse a comma-separated IP list. A bad entry is a startup error: silently
/// dropping it would quietly fall back to one shared bucket.
fn parse_trusted_proxies(raw: &str) -> anyhow::Result<Vec<std::net::IpAddr>> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse()
                .map_err(|_| anyhow::anyhow!("{TRUSTED_PROXIES_ENV}: `{s}` is not an IP address"))
        })
        .collect()
}

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
///
/// The server has no TLS stack, so a non-loopback bind is plaintext. It is
/// refused unless `allow_insecure` is set, which an operator does only when a
/// TLS-terminating reverse proxy (or private network) fronts the listener.
fn check_bind_policy(
    bind: &str,
    token_set: bool,
    db_url_set: bool,
    allow_insecure: bool,
) -> anyhow::Result<()> {
    if is_loopback_bind(bind) {
        return Ok(());
    }
    if !allow_insecure {
        anyhow::bail!(
            "refusing to serve PLAINTEXT HTTP on non-loopback address `{bind}`: this server has no TLS. \
             Terminate TLS in a reverse proxy and bind loopback, or, if the listener is only \
             reachable by that proxy, pass `{INSECURE_BIND_FLAG}` (or set `{INSECURE_BIND_ENV}=1`)"
        );
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
    let allow_insecure = std::env::args().any(|a| a == INSECURE_BIND_FLAG)
        || insecure_env_enabled(std::env::var(INSECURE_BIND_ENV).ok().as_deref());
    check_bind_policy(&bind, token_set, db_url.is_some(), allow_insecure)?;
    if allow_insecure && !is_loopback_bind(&bind) {
        tracing::warn!(
            "serving plaintext HTTP on `{bind}` ({INSECURE_BIND_ENV} / {INSECURE_BIND_FLAG}); \
             TLS must be terminated in front of this listener"
        );
    }
    let trusted_proxies =
        parse_trusted_proxies(&std::env::var(TRUSTED_PROXIES_ENV).unwrap_or_default())?;
    let proxy_header = parse_proxy_header(
        &trusted_proxies,
        std::env::var(TRUSTED_PROXY_HEADER_ENV).ok().as_deref(),
    )?;

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
    let app = with_rate_limit_trusted(router, trusted_proxies, proxy_header);
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
    use super::{
        check_bind_policy, insecure_env_enabled, is_loopback_bind, parse_proxy_header,
        parse_trusted_proxies,
    };

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
        // loopback: always allowed, opt-in irrelevant
        for (t, d, i) in [
            (false, false, false),
            (true, true, false),
            (false, false, true),
        ] {
            assert!(check_bind_policy(loopback, t, d, i).is_ok());
        }
        // non-loopback without the opt-in: refused even with token + db
        let plain = check_bind_policy(public, true, true, false)
            .unwrap_err()
            .to_string();
        assert!(
            plain.contains("PLAINTEXT") && plain.contains("--allow-insecure-bind"),
            "{plain}"
        );
        // with the opt-in: token AND db url still required
        let no_token = check_bind_policy(public, false, true, true)
            .unwrap_err()
            .to_string();
        assert!(no_token.contains("without"), "{no_token}");
        let no_db = check_bind_policy(public, true, false, true)
            .unwrap_err()
            .to_string();
        assert!(no_db.contains("ephemeral"), "{no_db}");
        assert!(check_bind_policy(public, true, true, true).is_ok());
    }

    #[test]
    fn insecure_env_values() {
        assert!(insecure_env_enabled(Some("1")));
        assert!(insecure_env_enabled(Some("TRUE")));
        assert!(insecure_env_enabled(Some(" Yes ")));
        assert!(!insecure_env_enabled(Some("0")));
        assert!(!insecure_env_enabled(Some("on")));
        assert!(!insecure_env_enabled(None));
    }

    #[test]
    fn proxy_header_required_with_trusted_proxies() {
        let p = parse_trusted_proxies("10.0.0.1").unwrap();
        assert!(parse_proxy_header(&p, None).is_err());
        assert!(parse_proxy_header(&p, Some(" ")).is_err());
        assert!(parse_proxy_header(&p, Some("x-real-ip")).is_err());
        assert!(parse_proxy_header(&p, Some("Forwarded")).is_ok());
        assert!(parse_proxy_header(&[], None).is_ok());
        assert!(parse_proxy_header(&[], Some("bogus")).is_err());
    }

    #[test]
    fn trusted_proxies_parse_or_fail_loudly() {
        assert!(parse_trusted_proxies("").unwrap().is_empty());
        assert_eq!(parse_trusted_proxies("10.0.0.1, ::1").unwrap().len(), 2);
        assert!(parse_trusted_proxies("10.0.0.1,nope").is_err());
    }
}
