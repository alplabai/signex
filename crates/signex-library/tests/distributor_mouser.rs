//! Mouser adapter integration tests using wiremock.

#![cfg(feature = "distributors-community")]

use std::future::Future;

use serde_json::json;
use signex_library::distributor::{DistributorAdapter, DistributorSource};
use signex_library::distributors::mouser::MouserAdapter;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture_response(mpn: &str) -> serde_json::Value {
    json!({
        "SearchResults": {
            "Parts": [
                {
                    "MouserPartNumber": "603-RC0805FR-0710KL",
                    "ManufacturerPartNumber": mpn,
                    "Manufacturer": "YAGEO",
                    "Description": "Thick Film Resistors - SMD 0805 10K Ohms 1%",
                    "DataSheetUrl": "https://www.mouser.com/datasheet/2/447/yag_s_a0001939810_1-2541572.pdf",
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
fn lookup_by_mpn_passes_apikey_query_param() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/keyword"))
                    .and(query_param("apiKey", "DEADBEEF-1234"))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .set_body_json(fixture_response("RC0805FR-0710KL")),
                    )
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = MouserAdapter::with_api_key(base, "DEADBEEF-1234", None);
            let parts = adapter.lookup_by_mpn("RC0805FR-0710KL").unwrap();
            assert_eq!(parts.len(), 1);
            let p = &parts[0];
            assert_eq!(p.mpn, "RC0805FR-0710KL");
            assert_eq!(p.manufacturer, "YAGEO");
            assert_eq!(p.stock, Some(12_345));
            assert_eq!(p.source, DistributorSource::Mouser);
            assert_eq!(
                p.extra.get("mouser_pn").map(String::as_str),
                Some("603-RC0805FR-0710KL")
            );
        },
    );
}

#[test]
fn http_401_surfaces_auth_error() {
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
            let err = adapter.lookup_by_mpn("X").unwrap_err();
            assert!(
                err.to_string().contains("auth"),
                "expected Auth error, got {err}"
            );
        },
    );
}

#[test]
fn empty_search_results_returns_empty_vec() {
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
            let parts = adapter.lookup_by_mpn("UNKNOWN").unwrap();
            assert!(parts.is_empty());
        },
    );
}

#[test]
#[ignore = "live API — requires Mouser API key + network"]
fn live_lookup_smoke() {
    use signex_library::distributors::keyring::KeyringStore;
    let store = KeyringStore::for_provider("mouser", "default");
    let _key = store.get_secret().expect("Mouser API key in keyring");
    let adapter = MouserAdapter::from_keyring(None);
    let _ = adapter.lookup_by_mpn("RC0805FR-0710KL").expect("network");
}
