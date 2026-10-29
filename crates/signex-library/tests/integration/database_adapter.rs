//! Wiremock-backed integration tests for `DatabaseAdapter`.
//!
//! Row CRUD speaks to the `/tables` and `/rows` routes; primitives to
//! `/symbols` / `/footprints` / `/sims`. The stubs follow the server's
//! contract; `signex-library-server/tests/database_adapter.rs` runs the
//! adapter against the real router, so a stub that drifts from the server
//! is caught there (#688).
//!
//! Mirrors the `distributor_*` test layout — a private tokio runtime
//! drives a `MockServer`; the adapter (which uses `reqwest::blocking`)
//! runs on a standard thread so its blocking calls don't deadlock the
//! runtime.

#![cfg(feature = "database")]

use std::future::Future;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::database::DatabaseAdapter;
use signex_library::component::{
    ComponentRow, DatasheetRef, PlmReserved, ROW_SCHEMA, ROW_SCHEMA_HEADER,
};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
use signex_library::lifecycle::LifecycleState;
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::{
    Footprint, PinDirection, PrimitiveKind, PrimitiveRef, SimKind, SimModel, Symbol, SymbolPin,
};
use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use uuid::Uuid;
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test bearer token — every request must carry it via `Authorization: Bearer`.
const TEST_TOKEN: &str = "wiremock-bearer-token";
const TEST_HOLDER: &str = "test@signex";

/// Spin up a wiremock `MockServer` on a private runtime, run the setup
/// closure to register expectations, then hand the adapter (built against
/// the server URL) to the synchronous test body on a fresh thread.
pub(super) fn with_mock_server<S, T>(setup: S, test: T)
where
    S: FnOnce(&MockServer) -> std::pin::Pin<Box<dyn Future<Output = ()> + '_>>,
    T: FnOnce(DatabaseAdapter) + Send + 'static,
{
    with_mock_server_version(
        serde_json::json!({ "row_schema": ROW_SCHEMA, "primitive_revisions": true }),
        setup,
        test,
    );
}

/// Like [`with_mock_server`], with `/version` answering `version_body`.
fn with_mock_server_version<S, T>(version_body: serde_json::Value, setup: S, test: T)
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
    rt.block_on(async {
        Mock::given(method("GET"))
            .and(path("/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(version_body))
            .mount(&server)
            .await;
    });
    rt.block_on(setup(&server));

    let adapter =
        DatabaseAdapter::with_token(server.uri(), TEST_TOKEN, TEST_HOLDER).expect("adapter");
    let handle = std::thread::spawn(move || test(adapter));
    handle.join().expect("test panicked");

    drop(server);
    drop(rt);
}

// ── Primitive CRUD over HTTP ─────────────────────────────────────────────

/// A footprint whose sketch holds an arc about (2, 3) with start (7, 3) and an
/// end point at (2, 10): off the radius-5 circle, as an older file stored it.
/// Returns the footprint plus the ids of the centre and end points.
fn off_circle_footprint() -> (Footprint, SketchEntityId, SketchEntityId) {
    let plane = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let pid = plane.id;
    let mut sketch = SketchData::default();
    sketch.planes.push(plane);
    let mut point = |x, y| {
        let id = SketchEntityId::new();
        sketch
            .entities
            .push(Entity::new(id, pid, EntityKind::Point { x, y }));
        id
    };
    let (c, s, e) = (point(2.0, 3.0), point(7.0, 3.0), point(2.0, 10.0));
    sketch.entities.push(Entity::new(
        SketchEntityId::new(),
        pid,
        EntityKind::Arc {
            center: c,
            start: s,
            end: e,
            sweep_ccw: true,
        },
    ));
    let mut fp = Footprint::empty("arc");
    fp.uuid = Uuid::now_v7();
    fp.sketch = Some(sketch);
    (fp, c, e)
}

#[test]
fn get_footprint_repairs_an_off_circle_arc_end_stored_remotely() {
    let (fp, c, e) = off_circle_footprint();
    let uuid = fp.uuid;
    let body = serde_json::to_value(&fp).unwrap();
    let path_str = format!("/footprints/{uuid}");

    with_mock_server(
        move |server| {
            let path_for_mock = path_str.clone();
            let body = body.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path(path_for_mock.as_str()))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .insert_header("etag", "\"1\"")
                            .set_body_json(body),
                    )
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let got = adapter.get_footprint(uuid).expect("get_footprint");
            let sketch = got.sketch.expect("sketch survives");
            let at = |id| {
                sketch
                    .entities
                    .iter()
                    .find_map(|en| match en.kind {
                        EntityKind::Point { x, y } if en.id == id => Some((x, y)),
                        _ => None,
                    })
                    .expect("point")
            };
            let (cx, cy) = at(c);
            let (ex, ey) = at(e);
            assert!(((ex - cx).hypot(ey - cy) - 5.0).abs() < 1e-9);
        },
    );
}

fn fixture_symbol() -> Symbol {
    let mut s = Symbol::empty("OPAMP-DUAL-8");
    s.pins.clear();
    let mut p = SymbolPin::new("1", "OUT");
    p.electrical = PinDirection::Output;
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
                    .and(query_param("library_id", Uuid::nil().to_string()))
                    .and(header(
                        "authorization",
                        format!("Bearer {TEST_TOKEN}").as_str(),
                    ))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .insert_header("etag", "\"7\"")
                            .set_body_json(body),
                    )
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let got = adapter.get_symbol_versioned(uuid).expect("get_symbol");
            assert_eq!(got.value.uuid, uuid);
            assert_eq!(got.value.name, "OPAMP-DUAL-8");
            assert_eq!(got.revision, Some(7), "the copy carries its revision");
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
                    .and(body_partial_json(serde_json::json!({
                        "library_id": Uuid::nil(),
                        "name": "OPAMP-DUAL-8",
                    })))
                    .respond_with(ResponseTemplate::new(201).insert_header("etag", "\"1\""))
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
                    .and(body_partial_json(serde_json::json!({
                        "library_id": Uuid::nil(),
                        "name": "LM358",
                    })))
                    .respond_with(ResponseTemplate::new(201).insert_header("etag", "\"1\""))
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
    // The server's list rows: `{ library_id, uuid, name }`, no kind.
    let body = serde_json::json!([
        { "library_id": Uuid::nil(), "uuid": Uuid::now_v7(), "name": "Alpha" },
        { "library_id": Uuid::nil(), "uuid": Uuid::now_v7(), "name": "Mu" },
    ]);

    with_mock_server(
        move |server| {
            let body = body.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/symbols"))
                    .and(query_param("library_id", Uuid::nil().to_string()))
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
            assert_eq!(got.len(), 2);
            assert_eq!(got[0].name, "Alpha");
            assert_eq!(got[1].name, "Mu");
            assert!(
                got.iter()
                    .all(|summary| summary.kind == PrimitiveKind::Symbol)
            );
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
                    .and(query_param("library_id", Uuid::nil().to_string()))
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

#[test]
fn save_symbol_versioned_puts_with_the_copys_revision() {
    let sym = fixture_symbol();
    let path_str = format!("/symbols/{}", sym.uuid);

    with_mock_server(
        move |server| {
            let path_for_mock = path_str.clone();
            Box::pin(async move {
                Mock::given(method("PUT"))
                    .and(path(path_for_mock.as_str()))
                    .and(header("if-match", "\"4\""))
                    .respond_with(ResponseTemplate::new(200).insert_header("etag", "\"5\""))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let saved = adapter
                .save_symbol_versioned(sym, Some(4), "edit")
                .expect("save");
            assert_eq!(saved, Some(5), "the next save of this copy bases on 5");
        },
    );
}

#[test]
fn a_stale_primitive_save_maps_to_primitive_changed_and_a_missing_base_names_the_primitive() {
    let sym = fixture_symbol();
    let path_str = format!("/symbols/{}", sym.uuid);

    with_mock_server(
        move |server| {
            let path_for_mock = path_str.clone();
            Box::pin(async move {
                Mock::given(method("PUT"))
                    .and(path(path_for_mock.as_str()))
                    .and(header("if-match", "\"1\""))
                    .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                        "error": "changed since it was read",
                        "current_revision": 2,
                    })))
                    .mount(server)
                    .await;
                Mock::given(method("PUT"))
                    .and(path(path_for_mock.as_str()))
                    .and(header("if-match", "\"9\""))
                    .respond_with(ResponseTemplate::new(428))
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let err = adapter
                .save_symbol_versioned(sym.clone(), Some(1), "edit")
                .unwrap_err();
            assert!(
                matches!(
                    err,
                    signex_library::LibraryError::PrimitiveChanged {
                        current_revision: 2,
                        ..
                    }
                ),
                "{err:?}"
            );
            assert!(err.to_string().contains("symbol"), "{err}");
            let err = adapter
                .save_symbol_versioned(sym, Some(9), "edit")
                .unwrap_err()
                .to_string();
            assert!(err.contains("symbol"), "{err}");
            assert!(!err.contains("row"), "{err}");
        },
    );
}

/// A server older than primitive revisions advertises no
/// `primitive_revisions` in `/version` and treats a `POST` as an upsert.
/// Reading or saving through it is refused before any request reaches it,
/// so no save can silently replace someone else's version (#1445).
#[test]
fn primitive_reads_and_saves_are_refused_against_a_server_without_revisions() {
    let sym = fixture_symbol();
    let uuid = sym.uuid;
    let body = serde_json::to_value(&sym).unwrap();
    let path_str = format!("/symbols/{uuid}");

    with_mock_server_version(
        serde_json::json!({ "row_schema": ROW_SCHEMA }),
        move |server| {
            let path_for_mock = path_str.clone();
            let body = body.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path(path_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(body))
                    .expect(0)
                    .mount(server)
                    .await;
                for verb in ["POST", "PUT"] {
                    Mock::given(method(verb))
                        .respond_with(ResponseTemplate::new(201))
                        .expect(0)
                        .mount(server)
                        .await;
                }
            })
        },
        move |adapter| {
            for err in [
                adapter.get_symbol(uuid).unwrap_err(),
                adapter.save_symbol(sym, "add").unwrap_err(),
            ] {
                let text = err.to_string();
                assert!(text.contains("too old"), "{text}");
            }
        },
    );
}

/// The server is replaced by an older one inside the guard's cache window:
/// the next GET carries no `ETag`, which drops the cached capability answer,
/// so the following save re-reads `/version` and never sends its `POST`.
#[test]
fn a_reply_without_an_etag_drops_the_cached_primitive_check() {
    let sym = fixture_symbol();
    let uuid = sym.uuid;
    let body = serde_json::to_value(&sym).unwrap();
    let path_str = format!("/symbols/{uuid}");

    with_mock_server_version(
        serde_json::json!({ "row_schema": ROW_SCHEMA }),
        move |server| {
            let path_for_mock = path_str.clone();
            let body = body.clone();
            Box::pin(async move {
                // The first /version answer is capable; later ones are not.
                Mock::given(method("GET"))
                    .and(path("/version"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(
                        serde_json::json!({ "row_schema": ROW_SCHEMA, "primitive_revisions": true }),
                    ))
                    .up_to_n_times(1)
                    .with_priority(1)
                    .mount(server)
                    .await;
                Mock::given(method("GET"))
                    .and(path(path_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(body))
                    .mount(server)
                    .await;
                Mock::given(method("POST"))
                    .and(path("/symbols"))
                    .respond_with(ResponseTemplate::new(201))
                    .expect(0)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let err = adapter.get_symbol(uuid).unwrap_err().to_string();
            assert!(err.contains("may have gone through"), "{err}");
            let err = adapter.save_symbol(sym, "add").unwrap_err().to_string();
            assert!(err.contains("too old"), "{err}");
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
pub(super) fn mk_row(pn: &str, class: &str) -> ComponentRow {
    let lib = Uuid::new_v4();
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new(class),
        datasheet: DatasheetRef::url("https://example.com"),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
        alternate_footprints: Vec::new(),
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
        server_revision: None,
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
    let mut served = row.clone();
    served.server_revision = Some(1);
    let body_for_get = serde_json::to_value(&served).unwrap();
    served.server_revision = Some(2);
    let body_for_put = serde_json::to_value(&served).unwrap();

    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            let read_path_for_mock = read_path.clone();
            let put_path_for_mock = put_path.clone();
            let del_path_for_mock = del_path.clone();
            let get_body = body_for_get.clone();
            let put_body = body_for_put.clone();
            Box::pin(async move {
                // The insert and the update check part-number uniqueness
                // against the library's rows first.
                Mock::given(method("GET"))
                    .and(path("/tables"))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_json(serde_json::json!(["resistors"])),
                    )
                    .mount(server)
                    .await;
                Mock::given(method("GET"))
                    .and(path("/tables/resistors"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
                    .mount(server)
                    .await;
                Mock::given(method("POST"))
                    .and(path("/tables/resistors/rows"))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .and(header(
                        "authorization",
                        format!("Bearer {TEST_TOKEN}").as_str(),
                    ))
                    .and(header("x-signex-message", "create R0805_10k"))
                    .and(header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string().as_str()))
                    .respond_with(ResponseTemplate::new(201).set_body_json(get_body.clone()))
                    .expect(1)
                    .mount(server)
                    .await;

                Mock::given(method("GET"))
                    .and(path(read_path_for_mock.as_str()))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(get_body))
                    .expect(2)
                    .mount(server)
                    .await;

                Mock::given(method("PUT"))
                    .and(path(put_path_for_mock.as_str()))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .and(header("x-signex-message", "update R0805_10k"))
                    .and(header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string().as_str()))
                    .and(header("if-match", "\"1\""))
                    .respond_with(ResponseTemplate::new(200).set_body_json(put_body))
                    .expect(1)
                    .mount(server)
                    .await;

                Mock::given(method("DELETE"))
                    .and(path(del_path_for_mock.as_str()))
                    .and(query_param("library_id", nil_for_mock.as_str()))
                    .and(header("x-signex-message", "drop R0805_10k"))
                    .and(header("if-match", "\"2\""))
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
            assert_eq!(got.server_revision, Some(1));
            let revision = adapter
                .update_row("resistors", got, "update R0805_10k")
                .expect("update_row");
            assert_eq!(revision, Some(2), "update_row reports the new revision");
            adapter
                .delete_row(
                    "resistors",
                    RowId::from_uuid(row_id),
                    revision,
                    "drop R0805_10k",
                )
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
    original.server_revision = Some(4);
    let url_path = format!("/tables/resistors/rows/{row_id_str}");
    let expected_body = serde_json::to_value(&original).unwrap();
    let mut stored = original.clone();
    stored.server_revision = Some(5);
    let reply = serde_json::to_value(&stored).unwrap();

    with_mock_server(
        move |server| {
            let nil_for_mock = nil.clone();
            let path_for_mock = url_path.clone();
            let body_for_mock = expected_body.clone();
            let reply = reply.clone();
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
                    .and(header("if-match", "\"4\""))
                    .respond_with(ResponseTemplate::new(200).set_body_json(reply))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let revision = adapter
                .update_row("resistors", original.clone(), "bump R10K")
                .expect("update_row");
            assert_eq!(revision, Some(5));
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

/// A 409 naming `current_revision` is a stale write: the row changed on
/// the server since this copy was read.
#[test]
fn database_update_row_409_maps_to_row_changed() {
    let mut row = mk_row("R10K", "resistor");
    row.server_revision = Some(1);
    let url_path = format!("/tables/resistors/rows/{}", row.row_id);
    with_mock_server(
        move |server| {
            Box::pin(async move {
                Mock::given(method("PUT"))
                    .and(path(url_path.as_str()))
                    .and(header("if-match", "\"1\""))
                    .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
                        "error": "row changed since it was read",
                        "current_revision": 3,
                    })))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let err = adapter.update_row("resistors", row, "edit").unwrap_err();
            assert!(
                matches!(
                    err,
                    signex_library::LibraryError::RowChanged {
                        current_revision: 3
                    }
                ),
                "{err:?}"
            );
            assert!(err.to_string().contains("reload to see their version"));
        },
    );
}

/// A write without a base revision is refused with 428, reported as a
/// backend error that names the missing `If-Match`.
#[test]
fn database_delete_row_428_maps_to_backend() {
    let row_id = RowId::new();
    let url_path = format!("/tables/resistors/rows/{row_id}");
    with_mock_server(
        move |server| {
            Box::pin(async move {
                Mock::given(method("DELETE"))
                    .and(path(url_path.as_str()))
                    .respond_with(ResponseTemplate::new(428))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let err = adapter
                .delete_row("resistors", row_id, None, "drop")
                .unwrap_err();
            assert!(
                matches!(&err, signex_library::LibraryError::Backend(m) if m.contains("If-Match")),
                "{err:?}"
            );
        },
    );
}

/// A server built before labelled documents stores the row without them
/// and echoes it that way; the save reports it instead of passing silently
/// (#1239 review).
#[test]
fn database_update_row_refuses_a_server_that_drops_documents() {
    let mut row = mk_row("U1", "opamp");
    row.server_revision = Some(1);
    row.set_documents(vec![
        signex_library::DatasheetDoc::new("TI", DatasheetRef::url("https://ti.example/u1.pdf")),
        signex_library::DatasheetDoc::new("Errata", DatasheetRef::url("https://ti.example/e.pdf")),
    ]);
    let mut echoed = row.clone();
    echoed.datasheet_label.clear();
    echoed.more_datasheets.clear();
    echoed.server_revision = Some(2);
    let reply = serde_json::to_value(&echoed).unwrap();
    let url_path = format!("/tables/ics/rows/{}", row.row_id);
    with_mock_server(
        move |server| {
            Box::pin(async move {
                Mock::given(method("PUT"))
                    .and(path(url_path.as_str()))
                    .respond_with(ResponseTemplate::new(200).set_body_json(reply))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let err = adapter
                .update_row("ics", row, "edit")
                .expect_err("documents were dropped");
            assert!(err.to_string().contains("too old"), "{err}");
        },
    );
}

/// A server reporting a lower (or no) `row_schema` would store a row
/// without the fields it does not know: the client refuses the write and
/// sends nothing.
#[test]
fn row_writes_are_refused_against_an_old_server() {
    for version_body in [
        serde_json::json!({ "name": "signex-library-server" }),
        serde_json::json!({ "row_schema": ROW_SCHEMA - 1 }),
    ] {
        with_mock_server_version(
            version_body,
            |server| {
                Box::pin(async move {
                    for verb in ["POST", "PUT"] {
                        Mock::given(method(verb))
                            .respond_with(ResponseTemplate::new(201))
                            .expect(0)
                            .mount(server)
                            .await;
                    }
                })
            },
            |adapter| {
                let row = mk_row("R-OLD", "resistor");
                let err = adapter
                    .insert_row("resistors", row.clone(), "m")
                    .unwrap_err();
                assert!(
                    matches!(&err, signex_library::LibraryError::Backend(m) if m.contains("too old")),
                    "{err:?}"
                );
                let err = adapter.update_row("resistors", row, "m").unwrap_err();
                assert!(
                    matches!(&err, signex_library::LibraryError::Backend(m) if m.contains("too old")),
                    "{err:?}"
                );
            },
        );
    }
}

fn row_with_alternate_footprint() -> ComponentRow {
    use signex_library::footprint_binding::FootprintBinding;
    let mut row = mk_row("U9", "opamp");
    row.server_revision = Some(1);
    row.footprint_ref = Some(PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7()));
    row.alternate_footprints = vec![FootprintBinding::new(PrimitiveRef::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
    ))];
    row
}

/// Run `insert_row` (or `update_row`) against a mock that echoes `row`,
/// with or without its alternate footprints, and return the call's result.
fn write_row_echoing(
    insert: bool,
    row: ComponentRow,
    strip_alternates: bool,
) -> Result<(), signex_library::LibraryError> {
    let mut echoed = row.clone();
    if strip_alternates {
        echoed.alternate_footprints.clear();
    }
    echoed.server_revision = Some(2);
    let reply = serde_json::to_value(&echoed).unwrap();
    let (verb, url_path, status) = if insert {
        ("POST", "/tables/ics/rows".to_string(), 201)
    } else {
        ("PUT", format!("/tables/ics/rows/{}", row.row_id), 200)
    };
    let outcome = std::sync::Arc::new(std::sync::Mutex::new(None));
    let slot = outcome.clone();
    with_mock_server(
        move |server| {
            Box::pin(async move {
                Mock::given(method(verb))
                    .and(path(url_path.as_str()))
                    .respond_with(ResponseTemplate::new(status).set_body_json(reply))
                    .expect(0..=1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let result = if insert {
                adapter.insert_row("ics", row, "write")
            } else {
                adapter.update_row("ics", row, "write").map(drop)
            };
            *slot.lock().unwrap() = Some(result);
        },
    );
    outcome.lock().unwrap().take().expect("test closure ran")
}

#[test]
fn a_server_that_drops_alternate_footprints_fails_insert_and_update() {
    for insert in [true, false] {
        let err = write_row_echoing(insert, row_with_alternate_footprint(), true)
            .expect_err("alternates were dropped");
        assert!(err.to_string().contains("alternate footprints"), "{err}");
    }
}

#[test]
fn an_identical_echo_keeps_insert_and_update_ok() {
    for insert in [true, false] {
        write_row_echoing(insert, row_with_alternate_footprint(), false)
            .unwrap_or_else(|e| panic!("insert={insert}: {e}"));
    }
}

#[test]
fn alternates_without_a_default_never_reach_the_server() {
    let mut row = row_with_alternate_footprint();
    row.footprint_ref = None;
    for insert in [true, false] {
        let err = write_row_echoing(insert, row.clone(), false).unwrap_err();
        assert!(err.to_string().contains("footprint_ref"), "{err}");
    }
}
