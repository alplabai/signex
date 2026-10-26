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
const MOUSER_FUZZY: &str = include_str!("fixtures/distributors/mouser_fuzzy_hits.json");
const MOUSER_KEYWORD_EUR: &str = include_str!("fixtures/distributors/mouser_keyword_eur.json");
const MOUSER_UNREADABLE: &str = include_str!("fixtures/distributors/mouser_unreadable_price.json");
const MOUSER_BAD_KEY: &str = include_str!("fixtures/distributors/mouser_errors_invalid_key.json");
const MOUSER_TOO_MANY: &str = include_str!("fixtures/distributors/mouser_errors_too_many.json");
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

/// Mount the partnumber and keyword endpoints; `keyword_calls` is how many
/// MPN searches the test allows.
async fn mount_mouser(
    server: &MockServer,
    partnumber: ResponseTemplate,
    keyword: ResponseTemplate,
    keyword_calls: u64,
) {
    Mock::given(method("POST"))
        .and(path("/api/v1/search/partnumber"))
        .respond_with(partnumber)
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/search/keyword"))
        .respond_with(keyword)
        .expect(keyword_calls)
        .mount(server)
        .await;
}

fn mouser(base: &str) -> MouserAdapter {
    MouserAdapter::with_api_key(format!("{base}/api/v1/search/keyword"), "k", None)
}

#[test]
fn mouser_unknown_sku_is_not_found_and_never_falls_back_to_the_mpn_search() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                // The SKU search answers with neighbours only; the MPN search
                // would find a real part, and must not be asked.
                mount_mouser(server, json(MOUSER_FUZZY), json(MOUSER_PARTNUMBER_USD), 0).await;
            })
        },
        |base| {
            let err = mouser(&base)
                .refresh_pricing(&part("RC0805FR-0710KL", Some("603-STALE")))
                .unwrap_err();
            assert!(matches!(err, DistributorError::NotFound), "{err:?}");
        },
    );
}

#[test]
fn mouser_sku_takes_the_exact_hit_not_the_last_fuzzy_one() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                mount_mouser(server, json(MOUSER_FUZZY), json("{}"), 0).await;
            })
        },
        |base| {
            let snap = mouser(&base)
                .refresh_pricing(&part("RC0805FR-0710KL", Some("603-rc0805fr-0710kl")))
                .unwrap();
            assert_eq!(snap.part_number.as_deref(), Some("603-RC0805FR-0710KL"));
            assert_eq!(snap.by_distributor["Mouser"][0].unit_price, 0.10);
        },
    );
}

#[test]
fn mouser_sku_lookup_failure_is_reported_not_swallowed() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                mount_mouser(
                    server,
                    ResponseTemplate::new(500),
                    json(MOUSER_PARTNUMBER_USD),
                    0,
                )
                .await;
            })
        },
        |base| {
            let err = mouser(&base)
                .refresh_pricing(&part("RC0805FR-0710KL", Some("603-RC0805FR-0710KL")))
                .unwrap_err();
            assert!(matches!(err, DistributorError::Backend(_)), "{err:?}");
        },
    );
}

fn mouser_keyword_only(server: &MockServer) -> std::pin::Pin<Box<dyn Future<Output = ()> + '_>> {
    Box::pin(async move {
        Mock::given(method("POST"))
            .and(path("/api/v1/search/keyword"))
            .respond_with(json(MOUSER_FUZZY))
            .expect(1)
            .mount(server)
            .await;
    })
}

#[test]
fn mouser_mpn_only_row_prices_the_exact_mpn_and_maker_and_records_the_part() {
    with_mock_server(mouser_keyword_only, |base| {
        let snap = mouser(&base)
            .refresh_pricing(&part("RC0805FR-0710KL", None))
            .unwrap();
        assert_eq!(snap.part_number.as_deref(), Some("603-RC0805FR-0710KL"));
        assert_eq!(snap.by_distributor["Mouser"].len(), 2);
    });
}

#[test]
fn mouser_mpn_only_row_with_no_exact_mpn_is_not_priced_from_the_last_hit() {
    with_mock_server(mouser_keyword_only, |base| {
        let err = mouser(&base)
            .refresh_pricing(&part("RC0805FR-0710", None))
            .unwrap_err();
        assert!(matches!(err, DistributorError::NotFound), "{err:?}");
    });
}

#[test]
fn mouser_mpn_only_row_of_another_maker_is_not_priced() {
    with_mock_server(mouser_keyword_only, |base| {
        let mut wanted = part("RC0805FR-0710KL", None);
        wanted.manufacturer = "Vishay".into();
        let err = mouser(&base).refresh_pricing(&wanted).unwrap_err();
        assert!(matches!(err, DistributorError::NotFound), "{err:?}");
    });
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
            let mut quote = part("QUOTE-ONLY-1", None);
            quote.manufacturer = "ACME".into();
            match a.refresh_pricing(&quote) {
                Err(DistributorError::Backend(m)) => {
                    assert!(m.contains("price unreadable") && m.contains("Call"), "{m}")
                }
                other => panic!("{other:?}"),
            }
        },
    );
}

fn mouser_error_for(body: &'static str) -> DistributorError {
    let (tx, rx) = std::sync::mpsc::channel();
    with_mock_server(
        move |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/api/v1/search/partnumber"))
                    .respond_with(json(body))
                    .mount(server)
                    .await;
            })
        },
        move |base| {
            let a = MouserAdapter::with_api_key(format!("{base}/api/v1/search/keyword"), "k", None);
            let p = part("M-1", Some("M-1"));
            tx.send(a.refresh_pricing(&p).unwrap_err()).unwrap();
        },
    );
    rx.recv().unwrap()
}

#[test]
fn mouser_http_200_with_errors_for_a_bad_key_is_auth_not_not_found() {
    assert!(matches!(
        mouser_error_for(MOUSER_BAD_KEY),
        DistributorError::Auth(m) if m.contains("Invalid unique identifier")
    ));
}

#[test]
fn mouser_http_200_with_errors_for_a_quota_is_rate_limited() {
    assert!(matches!(
        mouser_error_for(MOUSER_TOO_MANY),
        DistributorError::RateLimited { .. }
    ));
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
                    .and(header("X-DIGIKEY-Client-Id", "cid"))
                    .respond_with(json(DIGIKEY_DETAILS))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = DigiKeyAdapter::with_access_token(base, "tok", None)
                .with_client_id("cid")
                .with_currency("eur");
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
            let a = DigiKeyAdapter::with_access_token(base, "tok", None).with_client_id("cid");
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
            let a = DigiKeyAdapter::with_access_token(base, "tok", None).with_client_id("cid");
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
            let a = DigiKeyAdapter::with_access_token(base, "tok", None).with_client_id("cid");
            let snap = a.refresh_pricing(&part("RC0805FR-0710KL", None)).unwrap();
            assert_eq!(snap.by_distributor["DigiKey"].len(), 2);
        },
    );
}

#[test]
fn digikey_sku_details_404_is_not_found_and_never_falls_back_to_the_keyword_search() {
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
                    .expect(0)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = DigiKeyAdapter::with_access_token(base, "tok", None).with_client_id("cid");
            let err = a
                .refresh_pricing(&part("RC0805FR-0710KL", Some("311-10.0KCRTR-ND")))
                .unwrap_err();
            assert!(matches!(err, DistributorError::NotFound), "{err:?}");
        },
    );
}

#[test]
fn digikey_sku_details_failure_is_reported_not_swallowed() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("GET"))
                    .respond_with(ResponseTemplate::new(500))
                    .expect(1)
                    .mount(server)
                    .await;
                Mock::given(method("POST"))
                    .respond_with(json(DIGIKEY_KEYWORD))
                    .expect(0)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = DigiKeyAdapter::with_access_token(base, "tok", None).with_client_id("cid");
            let err = a
                .refresh_pricing(&part("RC0805FR-0710KL", Some("311-10.0KCRTR-ND")))
                .unwrap_err();
            assert!(matches!(err, DistributorError::Backend(_)), "{err:?}");
        },
    );
}

#[test]
fn digikey_mpn_only_row_must_match_the_mpn_exactly() {
    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/products/v4/search/keyword"))
                    .respond_with(json(DIGIKEY_KEYWORD))
                    .expect(2)
                    .mount(server)
                    .await;
            })
        },
        |base| {
            let a = DigiKeyAdapter::with_access_token(base, "tok", None).with_client_id("cid");
            let snap = a.refresh_pricing(&part("RC0805FR-0710KL", None)).unwrap();
            assert_eq!(snap.part_number.as_deref(), Some("311-10.0KCRCT-ND"));
            let err = a.refresh_pricing(&part("RC0805FR-0710", None)).unwrap_err();
            assert!(matches!(err, DistributorError::NotFound), "{err:?}");
        },
    );
}

#[test]
fn digikey_claims_prices_only_once_it_sends_the_client_id() {
    let bare = DigiKeyAdapter::with_access_token("http://unused.invalid", "tok", None);
    assert!(
        !bare.supports_pricing(),
        "DigiKey rejects calls without X-DIGIKEY-Client-Id"
    );
    assert!(bare.with_client_id("cid").supports_pricing());
}

// ── LCSC / JLCPCB ───────────────────────────────────────────────────────

#[test]
fn lcsc_and_jlcpcb_do_not_claim_prices_their_models_do_not_read() {
    use signex_library::distributors::{JlcpcbAdapter, LcscAdapter};
    assert!(!LcscAdapter::with_base_url("http://unused.invalid", None).supports_pricing());
    assert!(!JlcpcbAdapter::with_base_url("http://unused.invalid", None).supports_pricing());
}
