//! M11: wiremock-backed integration tests for `DatabaseAdapter`.
//!
//! Per `v0.9-refactor-2-plan.md` §8, the row CRUD lands in WS-3 once the
//! `/tables` and `/rows` routes are wired on the server. WS-1 keeps just
//! the primitive (`/symbols` / `/footprints` / `/sims`) coverage — those
//! routes are unchanged under the row model.
//!
//! Mirrors the `distributor_*` test layout — a private tokio runtime drives
//! a `MockServer`; the adapter (which uses `reqwest::blocking`) runs on a
//! standard thread so its blocking calls don't deadlock the runtime.

#![cfg(feature = "database")]

use std::future::Future;

use signex_library::adapter::{LibraryAdapter, PrimitiveSummary};
use signex_library::adapters::database::DatabaseAdapter;
use signex_library::param::ParamMap;
use signex_library::primitive::{
    PinElectricalType, PinOrientation, PrimitiveKind, SimKind, SimModel, Symbol, SymbolPin,
};
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test bearer token — every request must carry it via `Authorization: Bearer`.
const TEST_TOKEN: &str = "wiremock-bearer-token";
const TEST_HOLDER: &str = "test@signex";

/// Spin up a wiremock `MockServer` on a private runtime, run the setup
/// closure to register expectations, then hand the adapter (built against
/// the server URL) to the synchronous test body on a fresh thread.
fn with_mock_server<S, T>(setup: S, test: T)
where
    S: FnOnce(&MockServer) -> std::pin::Pin<Box<dyn Future<Output = ()> + '_>>,
    T: FnOnce(DatabaseAdapter) + Send + 'static,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let server = rt.block_on(MockServer::start());
    rt.block_on(setup(&server));

    let adapter =
        DatabaseAdapter::with_token(server.uri(), TEST_TOKEN, TEST_HOLDER).expect("adapter");
    let handle = std::thread::spawn(move || test(adapter));
    handle.join().expect("test panicked");

    drop(server);
    drop(rt);
}

// ── Primitive CRUD over HTTP ─────────────────────────────────────────────

fn fixture_symbol() -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        name: "OPAMP-DUAL-8".into(),
        anchor: [0.0, 0.0],
        pins: vec![SymbolPin {
            number: "1".into(),
            name: "OUT".into(),
            electrical: PinElectricalType::Output,
            position: [0.0, 0.0],
            orientation: PinOrientation::Right,
            length: 2.54,
        }],
        graphics: Vec::new(),
        schematic_params: ParamMap::new(),
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
    }
}

#[test]
fn get_symbol_round_trips_through_get_symbols_uuid() {
    let sym = fixture_symbol();
    let uuid = sym.uuid;
    let body = serde_json::to_value(&sym).unwrap();
    let path_str = format!("/symbols/{uuid}");

    with_mock_server(
        move |server| {
            let path_for_mock = path_str.clone();
            let body = body.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path(path_for_mock.as_str()))
                    .and(header(
                        "authorization",
                        format!("Bearer {TEST_TOKEN}").as_str(),
                    ))
                    .respond_with(ResponseTemplate::new(200).set_body_json(body))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let got = adapter.get_symbol(uuid).expect("get_symbol");
            assert_eq!(got.uuid, uuid);
            assert_eq!(got.name, "OPAMP-DUAL-8");
        },
    );
}

#[test]
fn save_symbol_posts_to_symbols_with_message_header() {
    let sym = fixture_symbol();

    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/symbols"))
                    .and(header(
                        "authorization",
                        format!("Bearer {TEST_TOKEN}").as_str(),
                    ))
                    .and(header("x-signex-message", "add OPAMP-DUAL-8"))
                    .respond_with(ResponseTemplate::new(201))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            adapter
                .save_symbol(sym, "add OPAMP-DUAL-8")
                .expect("save_symbol");
        },
    );
}

#[test]
fn save_sim_posts_to_sims_route() {
    let sim = SimModel {
        uuid: Uuid::now_v7(),
        name: "LM358".into(),
        kind: SimKind::Spice3,
        body: ".SUBCKT LM358\n.ENDS".into(),
        default_node_map: Default::default(),
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
    };

    with_mock_server(
        |server| {
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/sims"))
                    .and(header(
                        "authorization",
                        format!("Bearer {TEST_TOKEN}").as_str(),
                    ))
                    .respond_with(ResponseTemplate::new(201))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            adapter.save_sim(sim, "add LM358").expect("save_sim");
        },
    );
}

#[test]
fn list_symbols_round_trips_through_get_symbols() {
    let summaries = vec![
        PrimitiveSummary {
            uuid: Uuid::now_v7(),
            name: "Alpha".into(),
            kind: PrimitiveKind::Symbol,
            used_by_count: 3,
        },
        PrimitiveSummary {
            uuid: Uuid::now_v7(),
            name: "Mu".into(),
            kind: PrimitiveKind::Symbol,
            used_by_count: 0,
        },
    ];
    let body = serde_json::to_value(&summaries).unwrap();
    let expected_len = summaries.len();

    with_mock_server(
        move |server| {
            let body = body.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/symbols"))
                    .and(header(
                        "authorization",
                        format!("Bearer {TEST_TOKEN}").as_str(),
                    ))
                    .respond_with(ResponseTemplate::new(200).set_body_json(body))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let got = adapter.list_symbols().expect("list_symbols");
            assert_eq!(got.len(), expected_len);
            assert_eq!(got[0].name, "Alpha");
            assert_eq!(got[0].used_by_count, 3);
            assert_eq!(got[1].kind, PrimitiveKind::Symbol);
        },
    );
}

#[test]
fn get_symbol_404_maps_to_not_found() {
    let uuid = Uuid::now_v7();
    let path_str = format!("/symbols/{uuid}");

    with_mock_server(
        move |server| {
            let path_for_mock = path_str.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path(path_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(404))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let err = adapter.get_symbol(uuid).unwrap_err();
            assert!(matches!(err, signex_library::LibraryError::NotFound(_)));
        },
    );
}
