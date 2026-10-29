// Standalone binary: ci.yml runs it by name (`--test integration_db -- --ignored`).
//! Integration tests covering DB schema migrations + the `/tables`
//! and `/rows` HTTP routes for the DBLib row model.
//!
//! Default backend: in-memory SQLite. Postgres path is gated behind
//! `SIGNEX_TEST_PG_URL` env var so CI without Postgres still passes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use signex_library::alternate_symbol::AlternateSymbol;
use signex_library::component::{
    ComponentRow, DatasheetRef, PinPadOverride, PlmReserved, ROW_SCHEMA, ROW_SCHEMA_HEADER,
};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
use signex_library::lifecycle::LifecycleState;
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use signex_library_server::db::{AppState, RowInsert};
use signex_library_server::{API_TOKEN_ENV, router_with_state};
use tower::ServiceExt;
use uuid::Uuid;

/// Test-fixture bearer token. H1: every protected route in the test harness
/// must pass `Authorization: Bearer <TEST_BEARER>`. Set via `SIGNEX_API_TOKEN`
/// on the test process so `router_with_state` picks it up at construction.
const TEST_BEARER: &str = "test-bearer-token";

/// Install the test bearer-token env var. Called by every test before they
/// build a router; idempotent and side-effect-safe across parallel tests
/// because the value never changes.
fn ensure_test_token() {
    // SAFETY: `set_var` requires unsynchronised access on Unix; here all
    // tests set the same constant value, so racing writers cannot disagree.
    // Once stabilised we can switch to `std::env::set_var` directly.
    unsafe {
        std::env::set_var(API_TOKEN_ENV, TEST_BEARER);
    }
}

/// Build a fixture row for the `resistors` table — covers the full
/// `ComponentRow` shape so JSON round-trips exercise every nested type.
fn fixture_row(internal_pn: &str) -> ComponentRow {
    let lib = Uuid::now_v7();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(internal_pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::now_v7()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::now_v7())),
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::<PinPadOverride>::new(),
        primary_mpn: ManufacturerPart::draft("Acme", format!("MPN-{internal_pn}")),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: Utc::now(),
        updated: Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

async fn fresh_state() -> AppState {
    ensure_test_token();
    let state = AppState::new_sqlite_memory()
        .await
        .expect("sqlite memory state");
    state.migrate().await.expect("migrations apply");
    state
}

/// Build an `Authorization: Bearer <TEST_BEARER>` header value once.
fn bearer_header() -> String {
    format!("Bearer {TEST_BEARER}")
}

#[tokio::test]
async fn migrations_apply_cleanly() {
    let state = fresh_state().await;
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(state.pool().sqlite().expect("sqlite pool"))
            .await
            .unwrap();

    // The row table must exist alongside the primitive tables and
    // any legacy tables retained for forward-compat.
    for required in ["component_rows", "symbols", "footprints", "sims"] {
        assert!(
            tables.iter().any(|t| t == required),
            "missing table {required}; have {tables:?}"
        );
    }
}

#[tokio::test]
async fn route_tables_lists_empty() {
    // Fresh library → no rows → `GET /tables` returns `[]`.
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/tables?library_id={library_id}"))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let names: Vec<String> = serde_json::from_slice(&bytes).unwrap();
    assert!(names.is_empty());
}

#[tokio::test]
async fn route_post_row_then_get() {
    // POST a row to /tables/resistors/rows, then GET it back via
    // /tables/resistors/rows/{row_id}.
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let row = fixture_row("R0805_10k");
    let body = serde_json::to_vec(&row).unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tables/resistors/rows?library_id={library_id}"))
                .header("content-type", "application/json")
                .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string())
                .header("authorization", bearer_header())
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/tables/resistors/rows/{}?library_id={library_id}",
                    row.row_id
                ))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let got: ComponentRow = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got.server_revision, Some(1));
    assert_eq!(
        got,
        ComponentRow {
            server_revision: Some(1),
            ..row.clone()
        }
    );

    // List endpoint surfaces the inserted row.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/tables/resistors?library_id={library_id}"))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let listed: Vec<ComponentRow> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].server_revision, Some(1));
    assert_eq!(listed[0].row_id, row.row_id);

    // After at least one row exists, /tables surfaces the table name.
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/tables?library_id={library_id}"))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let names: Vec<String> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(names, vec!["resistors".to_string()]);
}

#[tokio::test]
async fn route_post_duplicate_row_conflicts_and_preserves_original() {
    // POST must be create-only. A second POST with the same row_id must
    // return 409 and must NOT overwrite the stored row (the old handler
    // silently upserted, letting one client clobber another's edit).
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let row1 = fixture_row("R0805_10k");
    let mut row2 = row1.clone();
    row2.internal_pn = InternalPn::new("R0805_CLOBBER");
    row2.version = "9.9.9".into();

    let post = |body: Vec<u8>| {
        Request::builder()
            .method("POST")
            .uri(format!("/tables/resistors/rows?library_id={library_id}"))
            .header("content-type", "application/json")
            .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string())
            .header("authorization", bearer_header())
            .body(Body::from(body))
            .unwrap()
    };

    let r1 = app
        .clone()
        .oneshot(post(serde_json::to_vec(&row1).unwrap()))
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::CREATED);

    // Second POST with the same row_id but different content → 409.
    let r2 = app
        .clone()
        .oneshot(post(serde_json::to_vec(&row2).unwrap()))
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::CONFLICT);

    // The stored row is still the original — nothing was clobbered.
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/tables/resistors/rows/{}?library_id={library_id}",
                    row1.row_id
                ))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let got: ComponentRow = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        got,
        ComponentRow {
            server_revision: Some(1),
            ..row1.clone()
        },
        "the original row must survive a conflicting POST"
    );
}

#[tokio::test]
async fn route_put_row_updates() {
    // POST a row, PUT a modified copy back, GET should return the modified
    // version.
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let row = fixture_row("R0805_10k");
    let row_id = row.row_id;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tables/resistors/rows?library_id={library_id}"))
                .header("content-type", "application/json")
                .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string())
                .header("authorization", bearer_header())
                .body(Body::from(serde_json::to_vec(&row).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let mut updated = row.clone();
    updated.internal_pn = InternalPn::new("R0805_10k_REV2");
    updated.state = LifecycleState::Deprecated;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/tables/resistors/rows/{row_id}?library_id={library_id}"
                ))
                .header("if-match", "\"1\"")
                .header("content-type", "application/json")
                .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string())
                .header("authorization", bearer_header())
                .body(Body::from(serde_json::to_vec(&updated).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/tables/resistors/rows/{row_id}?library_id={library_id}"
                ))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let got: ComponentRow = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got.internal_pn, InternalPn::new("R0805_10k_REV2"));
    assert_eq!(got.state, LifecycleState::Deprecated);
    assert_eq!(got.server_revision, Some(2), "the PUT bumped the revision");
}

#[tokio::test]
async fn route_delete_row() {
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let row = fixture_row("R0805_10k");
    let row_id = row.row_id;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tables/resistors/rows?library_id={library_id}"))
                .header("content-type", "application/json")
                .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string())
                .header("authorization", bearer_header())
                .body(Body::from(serde_json::to_vec(&row).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/tables/resistors/rows/{row_id}?library_id={library_id}"
                ))
                .header("if-match", "\"1\"")
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/tables/resistors/rows/{row_id}?library_id={library_id}"
                ))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn route_unauthenticated_returns_401() {
    // Hit `/tables` without an Authorization header — the bearer-token
    // layer rejects with 401 before the handler runs.
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/tables?library_id={library_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires SIGNEX_TEST_PG_URL"]
async fn postgres_migrations_apply_when_env_set() {
    let url = std::env::var("SIGNEX_TEST_PG_URL")
        .expect("SIGNEX_TEST_PG_URL must be set to run the ignored Postgres tests");
    let state = AppState::connect(&url).await.expect("pg connect");
    state.migrate().await.expect("pg migrations apply");
}

/// #1056: the store refuses a non-finite parameter instead of saving
/// `null`, on both insert and update, and keeps the stored row.
#[tokio::test]
async fn store_refuses_non_finite_parameter() {
    use signex_library::param::ParamValue;

    let state = fresh_state().await;
    let lib = Uuid::now_v7();
    let good = fixture_row("R1");
    assert_eq!(
        state.insert_row(lib, "resistors", &good).await.unwrap(),
        RowInsert::Created
    );

    let mut bad = fixture_row("R_BAD");
    bad.parameters
        .insert("tolerance".into(), ParamValue::Number(f64::NAN));
    let err = state.insert_row(lib, "resistors", &bad).await.unwrap_err();
    assert!(err.to_string().contains("tolerance"), "{err}");

    let mut upd = good.clone();
    upd.parameters
        .insert("tolerance".into(), ParamValue::Number(f64::INFINITY));
    assert!(state.update_row(lib, "resistors", &upd, 1).await.is_err());
    let stored = state
        .fetch_row(lib, "resistors", RowId::from_uuid(good.row_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.parameters, good.parameters);
}

/// POST `row` into `resistors` of `library_id`.
async fn post_row(app: &axum::Router, library_id: Uuid, row: &ComponentRow) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tables/resistors/rows?library_id={library_id}"))
                .header("content-type", "application/json")
                .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string())
                .header("authorization", bearer_header())
                .body(Body::from(serde_json::to_vec(row).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

/// A PUT of `row`, with `If-Match` set to `if_match` when given.
fn put_request(library_id: Uuid, row: &ComponentRow, if_match: Option<&str>) -> Request<Body> {
    let mut req = Request::builder()
        .method("PUT")
        .uri(format!(
            "/tables/resistors/rows/{}?library_id={library_id}",
            row.row_id
        ))
        .header("content-type", "application/json")
        .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string())
        .header("authorization", bearer_header());
    if let Some(v) = if_match {
        req = req.header("if-match", v);
    }
    req.body(Body::from(serde_json::to_vec(row).unwrap()))
        .unwrap()
}

/// #115: a row write without the base revision is refused with 428, and
/// the row is left as it was.
#[tokio::test]
async fn route_put_without_if_match_is_428() {
    let state = fresh_state().await;
    let app = router_with_state(state.clone());
    let library_id = Uuid::now_v7();
    let row = fixture_row("R-428");
    post_row(&app, library_id, &row).await;

    let mut edited = row.clone();
    edited.internal_pn = InternalPn::new("R-428-EDIT");
    let resp = app
        .clone()
        .oneshot(put_request(library_id, &edited, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PRECONDITION_REQUIRED);

    let delete = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/tables/resistors/rows/{}?library_id={library_id}",
            row.row_id
        ))
        .header("authorization", bearer_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(delete).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PRECONDITION_REQUIRED);

    let stored = state
        .fetch_row(library_id, "resistors", RowId::from_uuid(row.row_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.internal_pn, InternalPn::new("R-428"));
    assert_eq!(stored.server_revision, Some(1));
}

/// #115: two concurrent PUTs based on the same revision — exactly one
/// wins, the other gets 409 naming the revision the winner left.
#[tokio::test]
async fn concurrent_puts_with_the_same_base_have_one_winner() {
    let state = fresh_state().await;
    let app = router_with_state(state.clone());
    let library_id = Uuid::now_v7();
    let row = fixture_row("R-RACE");
    post_row(&app, library_id, &row).await;

    let mut a = row.clone();
    a.internal_pn = InternalPn::new("R-RACE-A");
    let mut b = row.clone();
    b.internal_pn = InternalPn::new("R-RACE-B");
    let (ra, rb) = tokio::join!(
        app.clone()
            .oneshot(put_request(library_id, &a, Some("\"1\""))),
        app.clone()
            .oneshot(put_request(library_id, &b, Some("\"1\""))),
    );
    let (ra, rb) = (ra.unwrap(), rb.unwrap());
    let mut statuses = [ra.status(), rb.status()];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);

    let (winner, loser) = if ra.status() == StatusCode::OK {
        ("R-RACE-A", rb)
    } else {
        ("R-RACE-B", ra)
    };
    let bytes = axum::body::to_bytes(loser.into_body(), 1 << 20)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["current_revision"], 2, "{body}");

    let stored = state
        .fetch_row(library_id, "resistors", RowId::from_uuid(row.row_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.internal_pn, InternalPn::new(winner));
    assert_eq!(stored.server_revision, Some(2));
}

/// A row POST with the given schema header (`None` = header absent).
fn post_request(library_id: Uuid, row: &ComponentRow, schema: Option<&str>) -> Request<Body> {
    let mut req = Request::builder()
        .method("POST")
        .uri(format!("/tables/resistors/rows?library_id={library_id}"))
        .header("content-type", "application/json")
        .header("authorization", bearer_header());
    if let Some(v) = schema {
        req = req.header(ROW_SCHEMA_HEADER, v);
    }
    req.body(Body::from(serde_json::to_vec(row).unwrap()))
        .unwrap()
}

/// A row write from a build that does not send, or sends a lower, row
/// schema is refused with 400 and the schema code before the database is touched.
#[tokio::test]
async fn row_writes_from_an_older_build_are_refused_as_row_schema_too_old() {
    let state = fresh_state().await;
    let app = router_with_state(state);
    let library_id = Uuid::now_v7();
    let row = fixture_row("R-OLD");
    let too_low = (ROW_SCHEMA - 1).to_string();

    for schema in [None, Some(too_low.as_str()), Some("not-a-number")] {
        let resp = app
            .clone()
            .oneshot(post_request(library_id, &row, schema))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{schema:?}");
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 16)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["code"], signex_library::ROW_SCHEMA_REFUSED_CODE);
    }

    // Nothing was stored: the same row still inserts with the current schema.
    let current = ROW_SCHEMA.to_string();
    let resp = app
        .clone()
        .oneshot(post_request(library_id, &row, Some(&current)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // PUT: stripped header refused, current accepted.
    let mut no_header = put_request(library_id, &row, Some("\"1\""));
    no_header.headers_mut().remove(ROW_SCHEMA_HEADER);
    let resp = app.clone().oneshot(no_header).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let mut low = put_request(library_id, &row, Some("\"1\""));
    low.headers_mut()
        .insert(ROW_SCHEMA_HEADER, too_low.parse().unwrap());
    let resp = app.clone().oneshot(low).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = app
        .oneshot(put_request(library_id, &row, Some("\"1\"")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

/// A row with alternate symbols survives POST then GET through the JSONB
/// payload.
#[tokio::test]
async fn alternate_symbols_round_trip_through_the_server() {
    let state = fresh_state().await;
    let app = router_with_state(state);
    let library_id = Uuid::now_v7();
    let mut row = fixture_row("R-ALT");
    row.alternate_symbols.push(AlternateSymbol::new(
        PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7()),
        "0.0.3",
        "IEC",
    ));
    post_row_with(&app, library_id, &row).await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/tables/resistors/rows/{}?library_id={library_id}",
                    row.row_id
                ))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let got: ComponentRow = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got.alternate_symbols, row.alternate_symbols);
}

#[tokio::test]
async fn description_round_trips_through_the_server() {
    let app = router_with_state(fresh_state().await);
    let library_id = Uuid::now_v7();
    let mut row = fixture_row("R-DESC");
    row.description = "100 nF 50 V X7R µ".into();
    post_row_with(&app, library_id, &row).await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/tables/resistors/rows/{}?library_id={library_id}",
                    row.row_id
                ))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let got: ComponentRow = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got.description, row.description);
}

async fn post_row_with(app: &axum::Router, library_id: Uuid, row: &ComponentRow) {
    let current = ROW_SCHEMA.to_string();
    let resp = app
        .clone()
        .oneshot(post_request(library_id, row, Some(&current)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

/// POST `row` into `table` of `library_id` and return the status.
async fn post_row_status(
    app: &axum::Router,
    library_id: Uuid,
    table: &str,
    row: &ComponentRow,
) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tables/{table}/rows?library_id={library_id}"))
                .header("content-type", "application/json")
                .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string())
                .header("authorization", bearer_header())
                .body(Body::from(serde_json::to_vec(row).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

/// #1613: two writers racing the same new part number (different rows,
/// different tables) - exactly one is created, the other gets 409, and the
/// library keeps one row with that PN. A different library may reuse it.
async fn assert_racing_same_pn_has_one_winner(state: AppState) {
    let app = router_with_state(state.clone());
    let library_id = Uuid::now_v7();
    let (a, b) = (fixture_row("R-1613"), fixture_row(" R-1613 "));
    let (ra, rb) = tokio::join!(
        post_row_status(&app, library_id, "resistors", &a),
        post_row_status(&app, library_id, "capacitors", &b),
    );
    let mut statuses = [ra, rb];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::CREATED, StatusCode::CONFLICT]);

    let tables = ["resistors", "capacitors"];
    let mut held = 0;
    for (table, row) in tables.iter().zip([&a, &b]) {
        let id = RowId::from_uuid(row.row_id);
        held += usize::from(
            state
                .fetch_row(library_id, table, id)
                .await
                .unwrap()
                .is_some(),
        );
    }
    assert_eq!(held, 1);

    // Other library, blank PNs and a different case are all free.
    let other = Uuid::now_v7();
    for (table, pn, lib) in [
        ("resistors", "R-1613", other),
        ("resistors", "", library_id),
        ("capacitors", "", library_id),
        ("resistors", "r-1613", library_id),
    ] {
        let status = post_row_status(&app, lib, table, &fixture_row(pn)).await;
        assert_eq!(status, StatusCode::CREATED, "{table} {pn:?}");
    }

    // A PUT that moves a row onto a taken PN is refused too, row unchanged.
    let mover = fixture_row("R-MOVE");
    post_row(&app, library_id, &mover).await;
    let mut moved = mover.clone();
    moved.internal_pn = InternalPn::new("R-1613");
    let resp = app
        .clone()
        .oneshot(put_request(library_id, &moved, Some("\"1\"")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let stored = state
        .fetch_row(library_id, "resistors", RowId::from_uuid(mover.row_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.internal_pn, InternalPn::new("R-MOVE"));
    assert_eq!(stored.server_revision, Some(1));
}

#[tokio::test]
async fn racing_posts_of_one_part_number_have_one_winner() {
    assert_racing_same_pn_has_one_winner(fresh_state().await).await;
}

#[tokio::test]
#[ignore = "requires SIGNEX_TEST_PG_URL"]
async fn postgres_racing_posts_of_one_part_number_have_one_winner() {
    ensure_test_token();
    let url = std::env::var("SIGNEX_TEST_PG_URL")
        .expect("SIGNEX_TEST_PG_URL must be set to run the ignored Postgres tests");
    let state = AppState::connect(&url).await.expect("pg connect");
    state.migrate().await.expect("pg migrations apply");
    // Fresh library ids per run, so a reused database stays valid.
    assert_racing_same_pn_has_one_winner(state).await;
}

async fn exec(state: &AppState, sql: &str) {
    match state.pool() {
        signex_library_server::db::DbPool::Sqlite(p) => {
            sqlx::query(sql).execute(p).await.map(|_| ())
        }
        signex_library_server::db::DbPool::Postgres(p) => {
            sqlx::query(sql).execute(p).await.map(|_| ())
        }
    }
    .unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Store `pn` as `row`'s part number the way an older server would have: no
/// uniqueness check, `pn_key` left blank.
async fn seed_row(
    state: &AppState,
    library: Uuid,
    mut row: ComponentRow,
    pn: &str,
) -> ComponentRow {
    row.internal_pn = InternalPn::new(pn);
    let payload = serde_json::to_string(&row).unwrap().replace('\'', "''");
    let pn_sql = pn.replace('\'', "''");
    exec(
        state,
        &format!(
            "INSERT INTO component_rows \
             (library_id, table_name, row_id, internal_pn, payload, created_at, updated_at) \
             VALUES ('{library}', 'resistors', '{}', '{pn_sql}', '{payload}', 'x', 'x')",
            row.row_id
        ),
    )
    .await;
    row
}

async fn stored_pn_key(state: &AppState, row: &ComponentRow) -> String {
    let sql = format!(
        "SELECT pn_key FROM component_rows WHERE row_id = '{}'",
        row.row_id
    );
    match state.pool() {
        signex_library_server::db::DbPool::Sqlite(p) => {
            sqlx::query_scalar(&sql).fetch_one(p).await.unwrap()
        }
        signex_library_server::db::DbPool::Postgres(p) => {
            sqlx::query_scalar(&sql).fetch_one(p).await.unwrap()
        }
    }
}

/// #1613: a database already holding rows with one part number (a padded
/// copy counts: tab and NBSP are white space to Rust) is refused by the
/// migration with every clash named and the operator's fix spelled out; no
/// row is lost, and after the fix is applied to the payload the migration
/// goes through, `pn_key` follows the payload, and a description-only PUT of
/// a padded, non-duplicate row is accepted.
async fn assert_migration_refuses_duplicates_then_accepts_the_fix(state: AppState) {
    state.migrate().await.expect("fresh migrate");
    exec(&state, "DROP INDEX idx_component_rows_pn_key").await;
    exec(&state, "DELETE FROM _sqlx_migrations WHERE version = 10").await;
    let pg = state.pool().postgres().is_some();
    let library = Uuid::now_v7();
    let mut rows = Vec::new();
    for pn in ["DUP-1", "DUP-1\t", "OK-1", "NB\u{a0}", "NB", "PAD\t"] {
        rows.push(seed_row(&state, library, fixture_row(pn), pn).await);
    }

    let err = state.migrate().await.expect_err("duplicates are refused");
    let msg = err.to_string();
    assert!(msg.contains("\"DUP-1\"") && msg.contains("\"NB\""), "{msg}");
    assert!(msg.contains(&library.to_string()), "{msg}");
    assert!(
        msg.contains(if pg { "jsonb_set" } else { "json_set" }),
        "{msg}"
    );
    assert!(!msg.contains("OK-1") && !msg.contains("PAD"), "{msg}");
    for row in &rows {
        let kept = state
            .fetch_row(library, "resistors", RowId::from_uuid(row.row_id))
            .await
            .unwrap();
        assert!(kept.is_some(), "no row may be lost");
    }

    // The documented fix: renumber in the payload (and the column).
    for (row, new_pn) in [(&rows[1], "DUP-2"), (&rows[3], "NB-2")] {
        let (set, updated) = if pg {
            (
                format!(
                    "jsonb_set(payload::jsonb, '{{internal_pn}}', to_jsonb('{new_pn}'::text))::text"
                ),
                "to_char(now() AT TIME ZONE 'utc', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"+00:00\"')",
            )
        } else {
            (
                format!("json_set(payload, '$.internal_pn', '{new_pn}')"),
                "strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')",
            )
        };
        exec(
            &state,
            &format!(
                "UPDATE component_rows SET payload = {set}, internal_pn = '{new_pn}', \
                 revision = revision + 1, updated_at = {updated} WHERE row_id = '{}'",
                row.row_id
            ),
        )
        .await;
    }
    state.migrate().await.expect("migrates after renumbering");
    let renumbered = state
        .fetch_row(library, "resistors", RowId::from_uuid(rows[1].row_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(renumbered.internal_pn, InternalPn::new("DUP-2"));
    assert_eq!(renumbered.server_revision, Some(2));
    assert_eq!(stored_pn_key(&state, &rows[1]).await, "DUP-2");
    assert_eq!(stored_pn_key(&state, &rows[5]).await, "PAD");

    // The padded row is alone in its library: editing only its description works.
    let app = router_with_state(state.clone());
    let mut edited = state
        .fetch_row(library, "resistors", RowId::from_uuid(rows[5].row_id))
        .await
        .unwrap()
        .unwrap();
    edited.description = "edited".into();
    let resp = app
        .oneshot(put_request(library, &edited, Some("\"1\"")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn migration_refuses_existing_duplicate_part_numbers_without_losing_rows() {
    assert_migration_refuses_duplicates_then_accepts_the_fix(fresh_state().await).await;
}

/// Same, on Postgres, in a schema of its own so the dropped index and seeded
/// duplicates never touch the other Postgres tests.
#[tokio::test]
#[ignore = "requires SIGNEX_TEST_PG_URL"]
async fn postgres_migration_refuses_existing_duplicate_part_numbers_without_losing_rows() {
    ensure_test_token();
    let url = std::env::var("SIGNEX_TEST_PG_URL")
        .expect("SIGNEX_TEST_PG_URL must be set to run the ignored Postgres tests");
    let schema = format!("pn_dup_{}", Uuid::now_v7().simple());
    let admin = AppState::connect(&url).await.expect("pg connect");
    exec(&admin, &format!("CREATE SCHEMA {schema}")).await;
    let sep = if url.contains('?') { '&' } else { '?' };
    let scoped = format!("{url}{sep}options=-c%20search_path%3D{schema}");
    let state = AppState::connect(&scoped).await.expect("pg connect");
    assert_migration_refuses_duplicates_then_accepts_the_fix(state).await;
    exec(&admin, &format!("DROP SCHEMA {schema} CASCADE")).await;
}

/// #1613: with the index in place, a row whose `pn_key` no longer matches its
/// payload (an older replica wrote it) is repaired at the next start; if the
/// repair would clash with another row, start-up is refused naming the clash
/// and changes nothing.
async fn assert_restart_repairs_or_refuses_a_stale_key(state: AppState) {
    state.migrate().await.expect("fresh migrate");
    let library = Uuid::now_v7();
    let app = router_with_state(state.clone());
    let (a, b) = (fixture_row("K-1"), fixture_row("K-2"));
    post_row(&app, library, &a).await;
    post_row(&app, library, &b).await;

    exec(
        &state,
        &format!(
            "UPDATE component_rows SET pn_key = '' WHERE row_id = '{}'",
            a.row_id
        ),
    )
    .await;
    state.migrate().await.expect("restart repairs a stale key");
    assert_eq!(stored_pn_key(&state, &a).await, "K-1");
    assert_eq!(stored_pn_key(&state, &b).await, "K-2");

    // b's payload now claims a's number while its key still says K-2.
    let pg = state.pool().postgres().is_some();
    let set = if pg {
        "jsonb_set(payload::jsonb, '{internal_pn}', to_jsonb('K-1'::text))::text"
    } else {
        "json_set(payload, '$.internal_pn', 'K-1')"
    };
    exec(
        &state,
        &format!(
            "UPDATE component_rows SET payload = {set} WHERE row_id = '{}'",
            b.row_id
        ),
    )
    .await;
    let err = state.migrate().await.expect_err("clash is refused");
    assert!(err.to_string().contains("\"K-1\""), "{err}");
    assert_eq!(
        stored_pn_key(&state, &b).await,
        "K-2",
        "a refusal changes nothing"
    );
}

#[tokio::test]
async fn restart_repairs_a_stale_part_number_key() {
    assert_restart_repairs_or_refuses_a_stale_key(fresh_state().await).await;
}

#[tokio::test]
#[ignore = "requires SIGNEX_TEST_PG_URL"]
async fn postgres_restart_repairs_a_stale_part_number_key() {
    ensure_test_token();
    let url = std::env::var("SIGNEX_TEST_PG_URL")
        .expect("SIGNEX_TEST_PG_URL must be set to run the ignored Postgres tests");
    let schema = format!("pn_key_{}", Uuid::now_v7().simple());
    let admin = AppState::connect(&url).await.expect("pg connect");
    exec(&admin, &format!("CREATE SCHEMA {schema}")).await;
    let sep = if url.contains('?') { '&' } else { '?' };
    let scoped = format!("{url}{sep}options=-c%20search_path%3D{schema}");
    let state = AppState::connect(&scoped).await.expect("pg connect");
    assert_restart_repairs_or_refuses_a_stale_key(state).await;
    exec(&admin, &format!("DROP SCHEMA {schema} CASCADE")).await;
}
