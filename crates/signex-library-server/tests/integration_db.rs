//! Integration tests covering DB schema migrations + the `/tables`
//! and `/rows` HTTP routes for the DBLib row model.
//!
//! Default backend: in-memory SQLite. Postgres path is gated behind
//! `SIGNEX_TEST_PG_URL` env var so CI without Postgres still passes.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use signex_library::component::{ComponentRow, DatasheetRef, PinPadOverride, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
use signex_library::lifecycle::LifecycleState;
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use signex_library_server::db::AppState;
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
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(internal_pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::now_v7()),
        footprint_ref: Some(PrimitiveRef::new(lib, Uuid::now_v7())),
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
    assert!(state.insert_row(lib, "resistors", &good).await.unwrap());

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
