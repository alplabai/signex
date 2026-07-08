//! Integration tests covering DB schema migrations + the `/tables`
//! and `/rows` HTTP routes for the DBLib row model.
//!
//! Default backend: in-memory SQLite. Postgres path is gated behind
//! `SIGNEX_TEST_PG_URL` env var so CI without Postgres still passes.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use signex_library::adapter::FieldSet;
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
    assert_eq!(got, row);

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
    assert_eq!(listed, vec![row.clone()]);

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
        got, row1,
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

// Lock tests — kept here because the lock manager keys off
// `RowId.as_uuid()` and the ergonomics are easiest to exercise from
// a single integration suite.

#[tokio::test]
async fn lock_contention_second_attempt_blocks_until_release() {
    let state = fresh_state().await;
    state.locks().set_idle_ttl(Duration::from_millis(200));

    let row_uuid = RowId::new().as_uuid();

    state
        .locks()
        .try_lock(row_uuid, FieldSet::Symbol, "alice")
        .expect("alice acquires");

    let err = state
        .locks()
        .try_lock(row_uuid, FieldSet::Symbol, "bob")
        .unwrap_err();
    assert!(matches!(
        err.kind,
        signex_library_server::locks::LockErrorKind::Held { .. }
    ));

    state
        .locks()
        .release(row_uuid, FieldSet::Symbol, "alice")
        .unwrap();
    state
        .locks()
        .try_lock(row_uuid, FieldSet::Symbol, "bob")
        .expect("bob acquires after release");

    state
        .locks()
        .try_lock(row_uuid, FieldSet::Footprint, "alice")
        .expect("different field-set is independent");
}

#[tokio::test]
async fn lock_contention_ttl_expiry_allows_takeover() {
    let state = fresh_state().await;
    state.locks().set_idle_ttl(Duration::from_millis(50));

    let row_uuid = RowId::new().as_uuid();
    state
        .locks()
        .try_lock(row_uuid, FieldSet::Symbol, "alice")
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    state
        .locks()
        .try_lock(row_uuid, FieldSet::Symbol, "bob")
        .expect("bob takes over after TTL");
}

#[tokio::test]
async fn locks_endpoint_returns_409_when_held() {
    let state = fresh_state().await;
    state.locks().set_idle_ttl(Duration::from_secs(60));
    let app = router_with_state(state);

    let row_id = RowId::new();

    let mk_req = |holder: &str| {
        Request::builder()
            .method("POST")
            .uri(format!("/rows/{row_id}/locks"))
            .header("content-type", "application/json")
            .header("authorization", bearer_header())
            .header("x-signex-holder", holder)
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({"field_set": "Symbol"})).unwrap(),
            ))
            .unwrap()
    };

    let r1 = app.clone().oneshot(mk_req("alice")).await.unwrap();
    assert_eq!(r1.status(), StatusCode::CREATED);

    let r2 = app.oneshot(mk_req("bob")).await.unwrap();
    assert_eq!(r2.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "requires SIGNEX_TEST_PG_URL"]
async fn postgres_migrations_apply_when_env_set() {
    let url = match std::env::var("SIGNEX_TEST_PG_URL") {
        Ok(u) => u,
        Err(_) => return, // Belt-and-braces — `#[ignore]` already skips by default.
    };
    let state = AppState::connect(&url).await.expect("pg connect");
    state.migrate().await.expect("pg migrations apply");
}
