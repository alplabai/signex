//! Wiremock-backed integration tests for `DatabaseAdapter`.
//!
//! Row CRUD speaks to the `/tables` and `/rows` routes; primitive
//! (`/symbols` / `/footprints` / `/sims`) coverage stays unchanged
//! under the DBLib model.
//!
//! Mirrors the `distributor_*` test layout — a private tokio runtime
//! drives a `MockServer`; the adapter (which uses `reqwest::blocking`)
//! runs on a standard thread so its blocking calls don't deadlock the
//! runtime.

#![cfg(feature = "database")]

use std::future::Future;

use signex_library::adapter::{LibraryAdapter, PrimitiveSummary};
use signex_library::adapters::database::DatabaseAdapter;
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
use signex_library::lifecycle::LifecycleState;
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::{
    PinElectricalType, PinOrientation, PrimitiveKind, PrimitiveRef, SimKind, SimModel, Symbol,
    SymbolPin,
};
use uuid::Uuid;
use wiremock::matchers::{header, method, path, query_param};
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
    let mut s = Symbol::empty("OPAMP-DUAL-8");
    s.pins.clear();
    let mut p = SymbolPin::new("1", "OUT");
    p.electrical = PinElectricalType::Output;
    s.pins.push(p);
    s
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
        version: "0.0.1".into(),
        released: false,
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

// ── Row CRUD over HTTP ────────────────────────────────────────────────────
//
// `DatabaseAdapter::with_token` fabricates a manifest whose `library_id` is
// `Uuid::nil()`; the wiremock expectations match that nil-uuid query string.
// The matching server-side routes live in `signex-library-server`.

/// Build a `ComponentRow` fixture — same shape as `component::tests::fixture_row`
/// but with controllable PN + class so the assertions in each test don't
/// fight over identity.
fn mk_row(pn: &str, class: &str) -> ComponentRow {
    let lib = Uuid::new_v4();
    let now = chrono::Utc::now();
    ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new(class),
        datasheet: DatasheetRef::url("https://example.com"),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Vishay", "CRCW08051002F"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    }
}

/// Round-trip: insert → read → update → delete, each call hitting its own
/// mock route. Mirrors the LocalGit test plan (`local_git_adapter.rs`)
/// per `v0.9-refactor-2-plan.md` §8 step 3.5.
#[test]
fn database_round_trip_row() {
    let row = mk_row("R0805_10k", "resistor");
    let row_id = row.row_id;
    let row_id_str = row_id.to_string();
    let nil = Uuid::nil().to_string();
    let read_path = format!("/tables/resistors/rows/{row_id_str}");
    let put_path = read_path.clone();
    let del_path = read_path.clone();
    let body_for_get = serde_json::to_value(&row).unwrap();

    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            let read_path_for_mock = read_path.clone();
            let put_path_for_mock = put_path.clone();
            let del_path_for_mock = del_path.clone();
            let get_body = body_for_get.clone();
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path("/tables/resistors/rows"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .and(header(
                        "authorization",
                        format!("Bearer {TEST_TOKEN}").as_str(),
                    ))
                    .and(header("x-signex-message", "create R0805_10k"))
                    .respond_with(ResponseTemplate::new(201))
                    .expect(1)
                    .mount(server)
                    .await;

                Mock::given(method("GET"))
                    .and(path(read_path_for_mock.as_str()))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(get_body))
                    .expect(1)
                    .mount(server)
                    .await;

                Mock::given(method("PUT"))
                    .and(path(put_path_for_mock.as_str()))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .and(header("x-signex-message", "update R0805_10k"))
                    .respond_with(ResponseTemplate::new(200))
                    .expect(1)
                    .mount(server)
                    .await;

                Mock::given(method("DELETE"))
                    .and(path(del_path_for_mock.as_str()))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .and(header("x-signex-message", "drop R0805_10k"))
                    .respond_with(ResponseTemplate::new(204))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            adapter
                .insert_row("resistors", row.clone(), "create R0805_10k")
                .expect("insert_row");
            let got = adapter
                .read_row("resistors", RowId::from_uuid(row_id))
                .expect("read_row");
            assert_eq!(got.row_id, row_id);
            assert_eq!(got.internal_pn.as_str(), "R0805_10k");
            adapter
                .update_row("resistors", row.clone(), "update R0805_10k")
                .expect("update_row");
            adapter
                .delete_row("resistors", RowId::from_uuid(row_id), "drop R0805_10k")
                .expect("delete_row");
        },
    );
}

#[test]
fn database_iter_rows_across_tables() {
    // iter_rows composes list_tables + read_table per plan §9 — the server
    // ships only the 6 row/table routes.
    let row_a = mk_row("R10K", "resistor");
    let row_b = mk_row("OPA177", "opamp");
    let nil = Uuid::nil().to_string();
    let tables = serde_json::json!(["resistors", "opamps"]);
    let resistors = serde_json::json!([row_a]);
    let opamps = serde_json::json!([row_b]);

    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            let tables = tables.clone();
            let resistors = resistors.clone();
            let opamps = opamps.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/tables"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(tables))
                    .expect(1)
                    .mount(server)
                    .await;
                Mock::given(method("GET"))
                    .and(path("/tables/resistors"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(resistors))
                    .expect(1)
                    .mount(server)
                    .await;
                Mock::given(method("GET"))
                    .and(path("/tables/opamps"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(opamps))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let rows = adapter.iter_rows().expect("iter_rows");
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].0, "resistors");
            assert_eq!(rows[0].1.internal_pn.as_str(), "R10K");
            assert_eq!(rows[1].0, "opamps");
            assert_eq!(rows[1].1.internal_pn.as_str(), "OPA177");
        },
    );
}

#[test]
fn database_read_row_by_pn() {
    // read_row_by_pn composes iter_rows then filters in-memory.
    let row = mk_row("R10K", "resistor");
    let row_for_assert = row.row_id;
    let nil = Uuid::nil().to_string();
    let tables = serde_json::json!(["resistors"]);
    let resistors = serde_json::json!([row]);

    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            let tables = tables.clone();
            let resistors = resistors.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/tables"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(tables))
                    .expect(1)
                    .mount(server)
                    .await;
                Mock::given(method("GET"))
                    .and(path("/tables/resistors"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(resistors))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let pn = InternalPn::new("R10K");
            let (table, got) = adapter.read_row_by_pn(&pn).expect("read_row_by_pn");
            assert_eq!(table, "resistors");
            assert_eq!(got.row_id, row_for_assert);
            assert_eq!(got.internal_pn.as_str(), "R10K");
        },
    );
}

#[test]
fn database_read_row_by_pn_404_maps_to_not_found() {
    // No matching row across all tables → NotFound.
    let nil = Uuid::nil().to_string();
    let tables = serde_json::json!(["resistors"]);
    let resistors: serde_json::Value = serde_json::json!([]);

    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            let tables = tables.clone();
            let resistors = resistors.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/tables"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(tables))
                    .expect(1)
                    .mount(server)
                    .await;
                Mock::given(method("GET"))
                    .and(path("/tables/resistors"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(resistors))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let err = adapter
                .read_row_by_pn(&InternalPn::new("UNKNOWN"))
                .unwrap_err();
            assert!(matches!(err, signex_library::LibraryError::NotFound(_)));
        },
    );
}

/// `update_row` carries a fresh `updated_at` and a new payload — the
/// adapter just forwards the row to the server, so the test verifies that
/// the PUT body matches what the caller sent (different `updated` from
/// the original `created`) and that the route round-trips successfully.
#[test]
fn database_update_row_modifies_payload() {
    let mut original = mk_row("R10K", "resistor");
    let original_created = original.created;
    let row_id = original.row_id;
    let row_id_str = row_id.to_string();
    let nil = Uuid::nil().to_string();
    // Mutate the payload — bump `updated`, change a parametric field.
    let later = original_created + chrono::Duration::seconds(1);
    original.updated = later;
    let url_path = format!("/tables/resistors/rows/{row_id_str}");
    let expected_body = serde_json::to_value(&original).unwrap();

    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            let path_for_mock = url_path.clone();
            let body_for_mock = expected_body.clone();
            Box::pin(async move {
                Mock::given(method("PUT"))
                    .and(path(path_for_mock.as_str()))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .and(header(
                        "authorization",
                        format!("Bearer {TEST_TOKEN}").as_str(),
                    ))
                    .and(header("x-signex-message", "bump R10K"))
                    .and(wiremock::matchers::body_json(body_for_mock))
                    .respond_with(ResponseTemplate::new(200))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            adapter
                .update_row("resistors", original.clone(), "bump R10K")
                .expect("update_row");
            // sanity check — the local row's `updated` strictly follows `created`
            assert!(original.updated > original_created);
        },
    );
}

#[test]
fn database_list_tables_returns_names() {
    let nil = Uuid::nil().to_string();
    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/tables"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .set_body_json(serde_json::json!(["resistors", "opamps"])),
                    )
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let names = adapter.list_tables().expect("list_tables");
            assert_eq!(names, vec!["resistors".to_string(), "opamps".to_string()]);
        },
    );
}

#[test]
fn database_read_table_returns_rows() {
    let row_a = mk_row("R10K", "resistor");
    let row_b = mk_row("R100", "resistor");
    let nil = Uuid::nil().to_string();
    let body = serde_json::to_value(vec![&row_a, &row_b]).unwrap();
    let row_a_pn = row_a.internal_pn.as_str().to_string();

    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            let body = body.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/tables/resistors"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(body))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let rows = adapter.read_table("resistors").expect("read_table");
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].internal_pn.as_str(), row_a_pn);
        },
    );
}

#[test]
fn database_read_row_404_maps_to_not_found() {
    let row_id = Uuid::now_v7();
    let row_id_str = row_id.to_string();
    let nil = Uuid::nil().to_string();
    let url_path = format!("/tables/resistors/rows/{row_id_str}");

    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            let path_for_mock = url_path.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path(path_for_mock.as_str()))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(404))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let err = adapter
                .read_row("resistors", RowId::from_uuid(row_id))
                .unwrap_err();
            assert!(matches!(err, signex_library::LibraryError::NotFound(_)));
        },
    );
}
