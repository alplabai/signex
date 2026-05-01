//! JLCPCB adapter integration tests using wiremock.

#![cfg(feature = "distributors-community")]

use std::future::Future;

use serde_json::json;
use signex_library::distributor::{DistributorAdapter, DistributorSource};
use signex_library::distributors::jlcpcb::JlcpcbAdapter;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture_response(mpn: &str) -> serde_json::Value {
    json!({
        "data": {
            "list": [
                {
                    "componentCode": "C17414",
                    "mfrPart": mpn,
                    "manufacturer": "YAGEO",
                    "describe": "10kΩ ±1% 0.125W chip resistor",
                    "stockCount": 999_999,
                    "dataManualUrl": "https://datasheet.lcsc.com/lcsc/RC0805FR-0710KL.pdf",
                    "componentSpecificationEn": "0805"
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

    let base = format!("{}/jlcpcb-search", server.uri());
    let handle = std::thread::spawn(move || test(base));
    handle.join().expect("test panicked");

    drop(server);
    drop(rt);
}

#[test]
fn lookup_by_mpn_parses_wiremock_fixture() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/jlcpcb-search"))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .set_body_json(fixture_response("RC0805FR-0710KL")),
                    )
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = JlcpcbAdapter::with_base_url(base, None);
            let parts = adapter.lookup_by_mpn("RC0805FR-0710KL").unwrap();
            assert_eq!(parts.len(), 1);
            let p = &parts[0];
            assert_eq!(p.mpn, "RC0805FR-0710KL");
            assert_eq!(p.manufacturer, "YAGEO");
            assert_eq!(p.stock, Some(999_999));
            assert_eq!(p.source, DistributorSource::Jlcpcb);
            assert_eq!(p.footprint_hint.as_deref(), Some("0805"));
            assert_eq!(
                p.extra.get("jlcpcb_component_code").map(String::as_str),
                Some("C17414")
            );
        },
    );
}

#[test]
fn lookup_by_mpn_handles_empty_list() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/jlcpcb-search"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "data": { "list": [] }
                    })))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = JlcpcbAdapter::with_base_url(base, None);
            let parts = adapter.lookup_by_mpn("UNKNOWN").unwrap();
            assert!(parts.is_empty());
        },
    );
}

#[test]
fn cache_hit_short_circuits_post() {
    let dir = tempfile::tempdir().unwrap();
    let cache =
        signex_library::distributors::cache::DistributorCache::with_root(dir.path()).unwrap();

    let pre = signex_library::distributor::DistributorPart {
        mpn: "RC0805FR-0710KL".into(),
        manufacturer: "Yageo".into(),
        description: "Pre-cached".into(),
        datasheet_url: None,
        footprint_hint: None,
        parameters: Default::default(),
        pricing: None,
        stock: Some(1),
        source: DistributorSource::Jlcpcb,
        captured_at: chrono::Utc::now(),
        extra: Default::default(),
    };
    cache.put("jlcpcb", &pre).unwrap();
    let cache_for_test = cache;

    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/jlcpcb-search"))
                    .respond_with(ResponseTemplate::new(500))
                    .expect(0)
                    .mount(server)
                    .await;
            })
        },
        move |base| {
            let adapter = JlcpcbAdapter::with_base_url(base, Some(cache_for_test));
            let parts = adapter.lookup_by_mpn("RC0805FR-0710KL").unwrap();
            assert_eq!(parts.len(), 1);
            assert_eq!(parts[0].description, "Pre-cached");
        },
    );
}

#[test]
#[ignore = "live API — requires network access"]
fn live_lookup_smoke() {
    let adapter = JlcpcbAdapter::new(None);
    let _ = adapter.lookup_by_mpn("RC0805FR-0710KL").expect("network");
}
