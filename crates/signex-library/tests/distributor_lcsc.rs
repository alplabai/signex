//! LCSC adapter integration tests using wiremock.

#![cfg(feature = "distributors-community")]

use std::future::Future;

use serde_json::json;
use signex_library::distributor::{DistributorAdapter, DistributorSource};
use signex_library::distributors::lcsc::LcscAdapter;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture_response(mpn: &str) -> serde_json::Value {
    json!({
        "result": {
            "productList": [
                {
                    "productCode": "C17414",
                    "productModel": mpn,
                    "brandNameEn": "YAGEO",
                    "productIntroEn": "10kΩ ±1% 0.125W ±100ppm/℃ 0805 Chip Resistor",
                    "stockNumber": 192_321,
                    "pdfUrl": "https://datasheet.lcsc.com/lcsc/RC0805FR-0710KL.pdf",
                    "encapStandard": "0805_2012Metric"
                }
            ]
        }
    })
}

/// Spawn a multi-thread Tokio runtime in a background thread so wiremock
/// has somewhere to live; the main test thread runs the blocking
/// `LcscAdapter` calls. Returns once `test` completes.
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

    let base = format!("{}/wmsc/product/list", server.uri());
    // Run the sync test on a fresh OS thread so reqwest::blocking can
    // build its own runtime without colliding with `rt`.
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
                Mock::given(method("GET"))
                    .and(path("/wmsc/product/list"))
                    .and(query_param("keyword", "RC0805FR-0710KL"))
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
            let adapter = LcscAdapter::with_base_url(base, None);
            let parts = adapter.lookup_by_mpn("RC0805FR-0710KL").unwrap();
            assert_eq!(parts.len(), 1);
            let p = &parts[0];
            assert_eq!(p.mpn, "RC0805FR-0710KL");
            assert_eq!(p.manufacturer, "YAGEO");
            assert!(p.description.starts_with("10k"));
            assert_eq!(p.stock, Some(192_321));
            assert_eq!(p.source, DistributorSource::Lcsc);
            assert_eq!(p.footprint_hint.as_deref(), Some("0805_2012Metric"));
            assert!(p.datasheet_url.is_some());
            assert_eq!(
                p.extra.get("lcsc_product_code").map(String::as_str),
                Some("C17414")
            );
        },
    );
}

#[test]
fn lookup_by_mpn_returns_empty_on_empty_product_list() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/wmsc/product/list"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "result": { "productList": [] }
                    })))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = LcscAdapter::with_base_url(base, None);
            let parts = adapter.lookup_by_mpn("UNKNOWN-MPN").unwrap();
            assert!(parts.is_empty());
        },
    );
}

#[test]
fn http_429_surfaces_rate_limited_with_retry_after() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/wmsc/product/list"))
                    .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "42"))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = LcscAdapter::with_base_url(base, None);
            let err = adapter.lookup_by_mpn("X").unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("rate-limited"),
                "expected rate-limited error, got: {msg}"
            );
        },
    );
}

#[test]
fn cache_hit_short_circuits_network() {
    let dir = tempfile::tempdir().unwrap();
    let cache =
        signex_library::distributors::cache::DistributorCache::with_root(dir.path()).unwrap();

    // Pre-seed cache with a known part — adapter must not hit the wiremock
    // server for the same MPN within TTL.
    let mpn = "RC0805FR-0710KL";
    let pre = signex_library::distributor::DistributorPart {
        mpn: mpn.into(),
        manufacturer: "Yageo".into(),
        description: "Pre-cached".into(),
        datasheet_url: None,
        footprint_hint: None,
        parameters: Default::default(),
        pricing: None,
        stock: Some(1),
        source: DistributorSource::Lcsc,
        captured_at: chrono::Utc::now(),
        extra: Default::default(),
    };
    cache.put("lcsc", &pre).unwrap();

    let cache_for_test = cache.clone();
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/wmsc/product/list"))
                    .respond_with(ResponseTemplate::new(500))
                    .expect(0)
                    .mount(server)
                    .await;
            })
        },
        move |base| {
            let adapter = LcscAdapter::with_base_url(base, Some(cache_for_test));
            let parts = adapter.lookup_by_mpn(mpn).unwrap();
            assert_eq!(parts.len(), 1);
            assert_eq!(parts[0].description, "Pre-cached");
        },
    );
}

#[test]
#[ignore = "live API — requires network access; opt-in via --ignored"]
fn live_lookup_smoke() {
    // Hits the real LCSC search endpoint. Skipped in CI.
    let adapter = LcscAdapter::new(None);
    let parts = adapter.lookup_by_mpn("RC0805FR-0710KL").expect("network");
    assert!(!parts.is_empty(), "live LCSC should return at least one hit");
}
