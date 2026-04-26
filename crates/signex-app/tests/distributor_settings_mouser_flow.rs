//! UI-WS7 — wiremock-backed validation for the Mouser test flow.
//!
//! The signex-app handler runs `MouserAdapter::lookup_by_mpn(SENTINEL)`
//! against the Mouser API; on success it writes the API key to the OS
//! keyring. This test mirrors `signex-library/tests/distributor_mouser.rs`
//! by hitting the same code path against a wiremock instance — proves
//! the app's choice of sentinel MPN + adapter wiring matches the
//! library-level integration shape.
//!
//! We don't test the keyring writeback here because it depends on a
//! real OS keyring backend (Windows Credential Manager / Secret
//! Service); the writeback path lives behind one extra `if Ok` arm in
//! the dispatcher and is covered by the underlying
//! `KeyringStore::set_secret` tests in `signex-library`.

use std::future::Future;

use serde_json::json;
use signex_library::distributor::DistributorAdapter;
use signex_library::distributors::mouser::MouserAdapter;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Sentinel MPN — must stay in sync with the constant in
/// `crates/signex-app/src/app/dispatch/library.rs::handle_library_settings_message`.
const SENTINEL_MPN: &str = "RC0805FR-0710KL";

fn fixture_response(mpn: &str) -> serde_json::Value {
    json!({
        "SearchResults": {
            "Parts": [
                {
                    "MouserPartNumber": "603-RC0805FR-0710KL",
                    "ManufacturerPartNumber": mpn,
                    "Manufacturer": "YAGEO",
                    "Description": "Thick Film Resistors - SMD 0805 10K Ohms 1%",
                    "DataSheetUrl": "https://example.com/datasheet.pdf",
                    "Availability": "12,345 In Stock"
                }
            ]
        }
    })
}

fn with_mock_server<S, T>(setup: S, test: T)
where
    S: FnOnce(&MockServer) -> std::pin::Pin<Box<dyn Future<Output = ()> + '_>>,
    T: FnOnce(String) + Send + 'static,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let server = rt.block_on(MockServer::start());
    rt.block_on(setup(&server));

    let base = format!("{}/api/v1/search/keyword", server.uri());
    let handle = std::thread::spawn(move || test(base));
    handle.join().expect("test panicked");

    drop(server);
    drop(rt);
}

#[test]
fn mouser_test_flow_uses_sentinel_mpn_and_apikey_header() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/keyword"))
                    .and(header("apiKey", "TEST-KEY-123"))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .set_body_json(fixture_response(SENTINEL_MPN)),
                    )
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = MouserAdapter::with_api_key(base, "TEST-KEY-123", None);
            let parts = adapter
                .lookup_by_mpn(SENTINEL_MPN)
                .expect("Mouser test request should succeed against the wiremock");
            assert_eq!(parts.len(), 1);
            assert_eq!(parts[0].mpn, SENTINEL_MPN);
        },
    );
}

#[test]
fn mouser_test_flow_surfaces_auth_error_on_401() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/keyword"))
                    .respond_with(ResponseTemplate::new(401))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = MouserAdapter::with_api_key(base, "BAD-KEY", None);
            let err = adapter
                .lookup_by_mpn(SENTINEL_MPN)
                .expect_err("401 must surface as Auth error");
            // The dispatcher converts the DistributorError to a String
            // for `MouserTestResult(Err(_))` — sanity-check the prefix
            // so the UI's "Failed: <reason>" line stays informative.
            let msg = err.to_string();
            assert!(
                msg.to_ascii_lowercase().contains("auth"),
                "expected 'auth' in error, got {msg:?}"
            );
        },
    );
}

#[test]
fn mouser_test_flow_handles_empty_search_results() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/keyword"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "SearchResults": { "Parts": [] }
                    })))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = MouserAdapter::with_api_key(base, "K", None);
            // Empty hits = the API responded but the sentinel MPN
            // wasn't on file. The dispatcher treats this as success
            // (the key obviously works) — so we want lookup_by_mpn
            // to return an empty Vec, not an error.
            let parts = adapter.lookup_by_mpn(SENTINEL_MPN).unwrap();
            assert!(parts.is_empty());
        },
    );
}
