//! The per-IP governor (HI-2) sits outside the base router, so the
//! `oneshot` tests never exercise it. Serve over a real socket and assert
//! that a burst past the limit is answered with 429.

use signex_library_server::{router, with_rate_limit};
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

    let client = reqwest::Client::new();
    let url = format!("http://{addr}/health");
    let mut statuses = Vec::new();
    // Burst size is 30 and refills 1/s; 60 back-to-back requests must trip it.
    for _ in 0..60 {
        statuses.push(client.get(&url).send().await.unwrap().status().as_u16());
    }
    assert_eq!(statuses[0], 200, "first request is within the burst");
    assert!(
        statuses.contains(&429),
        "no 429 after 60 rapid requests: {statuses:?}"
    );
}
