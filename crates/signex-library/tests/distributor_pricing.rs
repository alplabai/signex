//! Price parsing of the Mouser and DigiKey clients, against committed JSON
//! fixtures shaped like the vendors' documented responses (Mouser Search API
//! `PriceBreaks[]`; DigiKey Product Information v4 `ProductVariations[].
//! StandardPricing[]`). The numbers are synthetic.

#![cfg(feature = "distributors-community")]

use std::future::Future;

use signex_library::distributor::{DistributorAdapter, DistributorError, DistributorPart};
use signex_library::distributors::digikey::DigiKeyAdapter;
use signex_library::distributors::mouser::MouserAdapter;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MOUSER_PARTNUMBER_USD: &str =
    include_str!("fixtures/distributors/mouser_partnumber_usd.json");
const MOUSER_KEYWORD_EUR: &str = include_str!("fixtures/distributors/mouser_keyword_eur.json");
const MOUSER_UNREADABLE: &str = include_str!("fixtures/distributors/mouser_unreadable_price.json");
const DIGIKEY_DETAILS: &str = include_str!("fixtures/distributors/digikey_productdetails.json");
const DIGIKEY_KEYWORD: &str = include_str!("fixtures/distributors/digikey_keyword.json");

fn json(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_string(body)
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
    std::thread::spawn(move || test(base))
        .join()
        .expect("test panicked");
    drop(server);
    drop(rt);
}

fn part(mpn: &str, sku: Option<&str>) -> DistributorPart {
    DistributorPart {
        mpn: mpn.into(),
        manufacturer: "YAGEO".into(),
        description: String::new(),
        datasheet_url: None,
        footprint_hint: None,
        parameters: Default::default(),
        pricing: None,
        stock: None,
        source: signex_library::distributor::DistributorSource::Mouser,
        captured_at: chrono::Utc::now(),
        extra: sku
            .map(|s| [("sku".to_string(), s.to_string())].into())
            .unwrap_or_default(),
    }
}

// ── Mouser ──────────────────────────────────────────────────────────────

#[test]
fn mouser_refresh_by_sku_uses_the_partnumber_search_and_reads_breaks() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/partnumber"))
                    .and(header("apiKey", "k"))
                    .and(body_partial_json(serde_json::json!({
                        "SearchByPartRequest": { "mouserPartNumber": "603-RC0805FR-0710KL" }
                    })))
                    .respond_with(json(MOUSER_PARTNUMBER_USD))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = MouserAdapter::with_api_key(format!("{base}/api/v1/search/keyword"), "k", None);
            assert!(a.supports_pricing());
            let snap = a
                .refresh_pricing(&part("RC0805FR-0710KL", Some("603-RC0805FR-0710KL")))
                .unwrap();
            assert_eq!(snap.currency, "USD");
            assert_eq!(snap.stock, Some(12_345));
            let breaks: Vec<(u32, f64)> = snap.by_distributor["Mouser"]
                .iter()
                .map(|b| (b.qty, b.unit_price))
                .collect();
            assert_eq!(breaks, [(1, 0.10), (10, 0.058), (1000, 1234.50)]);
        },
    );
}

#[test]
fn mouser_unknown_sku_falls_back_to_the_mpn_search() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/partnumber"))
                    .respond_with(json(r#"{"SearchResults":{"Parts":[]}}"#))
                    .expect(1)
                    .mount(server)
                    .await;
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/keyword"))
                    .respond_with(json(MOUSER_PARTNUMBER_USD))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = MouserAdapter::with_api_key(format!("{base}/api/v1/search/keyword"), "k", None);
            let snap = a
                .refresh_pricing(&part("RC0805FR-0710KL", Some("603-STALE")))
                .unwrap();
            assert_eq!(snap.by_distributor["Mouser"].len(), 3);
        },
    );
}

#[test]
fn mouser_reads_comma_decimal_prices_by_currency() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/keyword"))
                    .respond_with(json(MOUSER_KEYWORD_EUR))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = MouserAdapter::with_api_key(format!("{base}/api/v1/search/keyword"), "k", None);
            let snap = a.refresh_pricing(&part("RC0805FR-0710KL", None)).unwrap();
            assert_eq!(snap.currency, "EUR");
            assert_eq!(snap.stock, Some(8000), "numeric AvailabilityInStock");
            let prices: Vec<f64> = snap.by_distributor["Mouser"]
                .iter()
                .map(|b| b.unit_price)
                .collect();
            assert_eq!(prices, [0.094, 0.041, 1234.56]);
        },
    );
}

#[test]
fn mouser_unreadable_price_is_a_named_error_not_a_zero() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/keyword"))
                    .respond_with(json(MOUSER_UNREADABLE))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = MouserAdapter::with_api_key(format!("{base}/api/v1/search/keyword"), "k", None);
            match a.refresh_pricing(&part("QUOTE-ONLY-1", None)) {
                Err(DistributorError::Backend(m)) => {
                    assert!(m.contains("price unreadable") && m.contains("Call"), "{m}")
                }
                other => panic!("{other:?}"),
            }
        },
    );
}

// ── DigiKey ─────────────────────────────────────────────────────────────

#[test]
fn digikey_refresh_by_sku_prices_that_packaging_in_the_requested_currency() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/products/v4/search/311-10.0KCRCT-ND/productdetails"))
                    .and(header("X-DIGIKEY-Locale-Currency", "EUR"))
                    .and(header("Authorization", "Bearer tok"))
                    .respond_with(json(DIGIKEY_DETAILS))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = DigiKeyAdapter::with_access_token(base, "tok", None).with_currency("eur");
            assert!(a.supports_pricing());
            let snap = a
                .refresh_pricing(&part("RC0805FR-0710KL", Some("311-10.0KCRCT-ND")))
                .unwrap();
            assert_eq!(snap.currency, "EUR");
            assert_eq!(snap.stock, Some(250_000));
            let breaks: Vec<(u32, f64)> = snap.by_distributor["DigiKey"]
                .iter()
                .map(|b| (b.qty, b.unit_price))
                .collect();
            assert_eq!(
                breaks,
                [(1, 0.10), (10, 0.058), (100, 0.0238)],
                "cut tape, not the reel"
            );
        },
    );
}

#[test]
fn digikey_reel_sku_gets_the_reel_breaks() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/products/v4/search/311-10.0KCRTR-ND/productdetails"))
                    .respond_with(json(DIGIKEY_DETAILS))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = DigiKeyAdapter::with_access_token(base, "tok", None);
            let snap = a
                .refresh_pricing(&part("RC0805FR-0710KL", Some("311-10.0KCRTR-ND")))
                .unwrap();
            assert_eq!(snap.currency, "USD", "default locale currency");
            assert_eq!(snap.by_distributor["DigiKey"][0].qty, 5000);
        },
    );
}

#[test]
fn digikey_sku_missing_from_the_variations_is_refused_not_mispriced() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("GET"))
                    .respond_with(json(DIGIKEY_DETAILS))
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = DigiKeyAdapter::with_access_token(base, "tok", None);
            match a.refresh_pricing(&part("RC0805FR-0710KL", Some("311-OTHER-ND"))) {
                Err(DistributorError::Backend(m)) => {
                    assert!(
                        m.contains("no product variation numbered 311-OTHER-ND"),
                        "{m}"
                    )
                }
                other => panic!("{other:?}"),
            }
        },
    );
}

#[test]
fn digikey_without_sku_uses_the_keyword_search_and_first_priced_variation() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/products/v4/search/keyword"))
                    .respond_with(json(DIGIKEY_KEYWORD))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = DigiKeyAdapter::with_access_token(base, "tok", None);
            let snap = a.refresh_pricing(&part("RC0805FR-0710KL", None)).unwrap();
            assert_eq!(snap.by_distributor["DigiKey"].len(), 2);
        },
    );
}

#[test]
fn digikey_details_404_falls_back_to_keyword_and_still_prices_the_sku_packaging() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("GET"))
                    .respond_with(ResponseTemplate::new(404))
                    .expect(1)
                    .mount(server)
                    .await;
                Mock::given(method("POST"))
                    .and(path("/products/v4/search/keyword"))
                    .respond_with(json(DIGIKEY_KEYWORD))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = DigiKeyAdapter::with_access_token(base, "tok", None);
            let snap = a
                .refresh_pricing(&part("RC0805FR-0710KL", Some("311-10.0KCRTR-ND")))
                .unwrap();
            assert_eq!(
                snap.by_distributor["DigiKey"][0].qty, 5000,
                "reel, not the first variation"
            );
        },
    );
}

// ── LCSC / JLCPCB ───────────────────────────────────────────────────────

#[test]
fn lcsc_and_jlcpcb_do_not_claim_prices_their_models_do_not_read() {
    use signex_library::distributors::{JlcpcbAdapter, LcscAdapter};
    assert!(!LcscAdapter::with_base_url("http://unused.invalid", None).supports_pricing());
    assert!(!JlcpcbAdapter::with_base_url("http://unused.invalid", None).supports_pricing());
}
