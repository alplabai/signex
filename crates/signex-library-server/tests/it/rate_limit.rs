//! The per-IP governor (HI-2) sits outside the base router, so the
//! `oneshot` tests never exercise it. Serve over a real socket and assert
//! that a burst past the limit is answered with 429.

use signex_library_server::{RATE_LIMIT_BURST_SIZE, router, with_rate_limit};
use std::net::SocketAddr;

#[tokio::test]
async fn burst_beyond_limit_is_rejected_with_429() {
    let app = with_rate_limit(router());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    let client = reqwest::Client::builder()
        .no_proxy() // loopback requests must not detour via HTTP_PROXY
        .build()
        .unwrap();
    let url = format!("http://{addr}/health");
    // Fire well past the burst back-to-back (refill is 1/s, so a few extra
    // tokens may trickle in on a slow machine; only the first BURST are
    // guaranteed 200 and the request right after the burst is rejected).
    let burst = RATE_LIMIT_BURST_SIZE as usize;
    let mut statuses = Vec::new();
    for _ in 0..burst + 1 {
        statuses.push(client.get(&url).send().await.unwrap().status().as_u16());
    }
    assert!(
        statuses[..burst].iter().all(|s| *s == 200),
        "first {burst} requests must be within the burst: {statuses:?}"
    );
    assert_eq!(
        statuses[burst],
        429,
        "request {} must be rejected",
        burst + 1
    );
}
