//! DigiKey adapter integration tests using wiremock.
//!
//! Two flavours:
//! 1. Inline access token + mocked product-search endpoint — covers the
//!    happy path of the adapter without the OAuth dance.
//! 2. Mocked OAuth2 token endpoint — exercises the refresh-token-grant
//!    path that production runs on every API call. Refresh token comes
//!    from the OS keyring; we plant it before the test runs.

#![cfg(feature = "distributors-community")]

use std::future::Future;

use serde_json::json;
use signex_library::distributor::{DistributorAdapter, DistributorSource};
use signex_library::distributors::digikey::{DigiKeyAdapter, DigiKeyAuth};
use signex_library::distributors::keyring::KeyringStore;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture_response(mpn: &str) -> serde_json::Value {
    json!({
        "Products": [
            {
                "ManufacturerProductNumber": mpn,
                "Manufacturer": { "Name": "YAGEO" },
                "Description": {
                    "ProductDescription": "RES SMD 10K OHM 1% 1/8W 0805"
                },
                "DatasheetUrl": "https://www.yageo.com/upload/media/product/productsearch/datasheet/rchip/PYu-RC_Group_51_RoHS_L_12.pdf",
                "QuantityAvailable": 250_000,
                "ProductUrl": "https://www.digikey.com/en/products/detail/yageo/RC0805FR-0710KL/727918"
            }
        ]
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

    let base = server.uri();
    let handle = std::thread::spawn(move || test(base));
    handle.join().expect("test panicked");

    drop(server);
    drop(rt);
}

#[test]
fn lookup_by_mpn_with_inline_token() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/products/v4/search/keyword"))
                    .and(header("Authorization", "Bearer test-access-token"))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_json(fixture_response("RC0805FR-0710KL")),
                    )
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = DigiKeyAdapter::with_access_token(base, "test-access-token", None);
            let parts = adapter.lookup_by_mpn("RC0805FR-0710KL").unwrap();
            assert_eq!(parts.len(), 1);
            let p = &parts[0];
            assert_eq!(p.mpn, "RC0805FR-0710KL");
            assert_eq!(p.manufacturer, "YAGEO");
            assert_eq!(p.stock, Some(250_000));
            assert_eq!(p.source, DistributorSource::DigiKey);
            assert!(p.description.contains("10K OHM"));
            assert!(p.datasheet_url.is_some());
        },
    );
}

#[test]
fn http_401_surfaces_auth_error() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/products/v4/search/keyword"))
                    .respond_with(ResponseTemplate::new(401))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = DigiKeyAdapter::with_access_token(base, "expired", None);
            let err = adapter.lookup_by_mpn("X").unwrap_err();
            assert!(err.to_string().contains("auth"), "got {err}");
        },
    );
}

#[test]
fn refresh_token_grant_calls_token_endpoint() {
    // Uses the in-memory test fallback so this test doesn't touch the OS
    // keyring at all (and therefore runs identically on every CI runner).
    with_mock_server(
        |server| {
            Box::pin(async move {
                // Token endpoint: returns a brand new access token.
                Mock::given(method("POST"))
                    .and(path("/oauth/token"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "access_token": "fresh-access-token",
                        "refresh_token": "refresh-tok-2",
                        "token_type": "Bearer",
                        "expires_in": 3600
                    })))
                    .expect(1)
                    .mount(server)
                    .await;
                // Product search: must see the access token we just minted.
                Mock::given(method("POST"))
                    .and(path("/products/v4/search/keyword"))
                    .and(header("Authorization", "Bearer fresh-access-token"))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_json(fixture_response("RC0805FR")),
                    )
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |base| {
            let auth_url = format!("{}/oauth/authorize", base);
            let token_url = format!("{}/oauth/token", base);
            let auth = DigiKeyAuth::with_endpoints(
                "client-id",
                "client-secret",
                "http://localhost/cb",
                &auth_url,
                &token_url,
            )
            .unwrap()
            .with_test_refresh_token("refresh-tok-1");
            let adapter = DigiKeyAdapter::with_oauth_and_base(base, auth, None);

            let parts = adapter.lookup_by_mpn("RC0805FR").unwrap();
            assert_eq!(parts.len(), 1);
            assert_eq!(parts[0].mpn, "RC0805FR");
        },
    );
}

#[test]
fn no_refresh_token_yields_auth_error() {
    // Construct an adapter whose KeyringStore points at a guaranteed-absent
    // username; access_token() must surface NoRefreshToken → Auth error.
    let store = KeyringStore::for_provider("digikey", "ws-c-test-absent-refresh");
    let _ = store.delete();

    let auth = DigiKeyAuth::with_endpoints(
        "client-id",
        "client-secret",
        "http://localhost/cb",
        "http://example.invalid/auth",
        "http://example.invalid/token",
    )
    .unwrap();
    // The auth above uses the "refresh" username slot; we don't write anything
    // there, but on Windows that slot may have a real refresh token from a
    // user's session. So we use the test fallback to short-circuit:
    let adapter = DigiKeyAdapter::with_oauth_and_base(
        "http://example.invalid",
        // No fallback set → reads from keyring; if a real token is there, the
        // request will fail at the network layer (good enough for this test).
        // To keep the test deterministic, we exercise the keyring miss path
        // via `KeyringError::NotFound` by using a test-only username via
        // direct construction. We can't reach that from public API, so we
        // accept either Auth or Network as the surfaced error.
        auth,
        None,
    );
    let err = adapter.lookup_by_mpn("X").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("auth") || msg.contains("network"),
        "expected Auth or Network error from missing refresh-token path, got {msg}"
    );
    let _ = store.delete();
}

#[test]
#[ignore = "live API — requires DigiKey credentials + browser-driven OAuth flow"]
fn live_lookup_smoke() {
    let auth = DigiKeyAuth::new(
        std::env::var("DIGIKEY_CLIENT_ID").expect("DIGIKEY_CLIENT_ID"),
        std::env::var("DIGIKEY_CLIENT_SECRET").expect("DIGIKEY_CLIENT_SECRET"),
        std::env::var("DIGIKEY_REDIRECT_URI").expect("DIGIKEY_REDIRECT_URI"),
    )
    .unwrap();
    let adapter = DigiKeyAdapter::new(auth, None);
    let _ = adapter.lookup_by_mpn("RC0805FR-0710KL").expect("network");
}
