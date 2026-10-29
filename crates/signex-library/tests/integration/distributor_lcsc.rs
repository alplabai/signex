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
    assert!(
        !parts.is_empty(),
        "live LCSC should return at least one hit"
    );
}

/// Counts `warn!` events emitted on the current thread while `f` runs.
fn count_warnings(f: impl FnOnce()) -> usize {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tracing::span::{Attributes, Id, Record};
    use tracing::{Event, Level, Metadata, Subscriber};

    struct Count(Arc<AtomicUsize>);
    impl Subscriber for Count {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &Attributes<'_>) -> Id {
            Id::from_u64(1)
        }
        fn record(&self, _: &Id, _: &Record<'_>) {}
        fn record_follows_from(&self, _: &Id, _: &Id) {}
        fn event(&self, event: &Event<'_>) {
            if *event.metadata().level() == Level::WARN {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        fn enter(&self, _: &Id) {}
        fn exit(&self, _: &Id) {}
    }

    let count = Arc::new(AtomicUsize::new(0));
    tracing::subscriber::with_default(Count(count.clone()), f);
    count.load(Ordering::SeqCst)
}

/// #806 at the adapter: a corrupt cache entry is logged and treated as a
/// miss, so `lookup_by_mpn` falls through to the live API instead of
/// failing or staying silent.
#[test]
fn a_corrupt_cache_entry_falls_through_to_the_live_lookup_and_warns() {
    let dir = tempfile::tempdir().unwrap();
    let cache =
        signex_library::distributors::cache::DistributorCache::with_root(dir.path()).unwrap();
    let mpn = "RC0805FR-0710KL";
    let entry = cache.entry_path("lcsc", mpn);
    std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
    std::fs::write(&entry, b"{").unwrap();

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
        move |base| {
            let adapter = LcscAdapter::with_base_url(base, Some(cache));
            let mut parts = Vec::new();
            let warned = count_warnings(|| parts = adapter.lookup_by_mpn(mpn).unwrap());
            assert_eq!(parts.len(), 1, "the live result is returned");
            assert_eq!(parts[0].mpn, mpn);
            assert_eq!(warned, 1, "the corrupt entry must be logged once");
        },
    );
}

fn part_for_refresh() -> signex_library::distributor::DistributorPart {
    signex_library::distributor::DistributorPart {
        mpn: "RC0805FR-0710KL".into(),
        manufacturer: "YAGEO".into(),
        description: String::new(),
        datasheet_url: None,
        footprint_hint: None,
        parameters: Default::default(),
        pricing: None,
        stock: None,
        source: DistributorSource::Lcsc,
        captured_at: chrono::Utc::now(),
        extra: Default::default(),
    }
}

#[test]
fn lookup_by_url_picks_the_hit_with_the_url_code_not_the_last() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/wmsc/product/list"))
                    .and(query_param("keyword", "C17414"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "result": { "productList": [
                            { "productCode": "C17414", "productModel": "RC0805FR-0710KL" },
                            { "productCode": "C99999", "productModel": "OTHER-PART" }
                        ] }
                    })))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let adapter = LcscAdapter::with_base_url(base, None);
            let url = url::Url::parse(
                "https://www.lcsc.com/product-detail/Resistors_YAGEO-RC0805FR-0710KL_C17414.html",
            )
            .unwrap();
            let part = adapter.lookup_by_url(&url).unwrap().unwrap();
            assert_eq!(part.mpn, "RC0805FR-0710KL");
        },
    );
}

#[test]
fn refresh_pricing_reports_that_lcsc_has_no_pricing() {
    // No mock mounted: the refusal must not need the network.
    let adapter = LcscAdapter::with_base_url("http://unused.invalid", None);
    let err = adapter.refresh_pricing(&part_for_refresh()).unwrap_err();
    assert!(
        err.to_string().contains("pricing not available from LCSC"),
        "{err}"
    );
}
