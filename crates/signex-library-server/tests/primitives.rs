//! Integration tests for the v0.9 library refactor primitive routes (WS-D).
//!
//! Each test exercises a `POST` → `GET` round-trip via `tower::ServiceExt`
//! against the in-memory test harness, exactly mirroring the flow that the
//! `LibraryAdapter` will use in production. Auth is the same fixture bearer
//! token used by `tests/integration_db.rs`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use signex_library::primitive::{Footprint, SimKind, SimModel, Symbol};
use signex_library_server::db::{AppState, PrimitiveSummary};
use signex_library_server::{API_TOKEN_ENV, router_with_state};
use tower::ServiceExt;
use uuid::Uuid;

/// Same fixture token used by `tests/integration_db.rs`. Setting this matches
/// the bearer-token expectation that `router_with_state` installs at
/// construction time (gating every primitive route).
const TEST_BEARER: &str = "test-bearer-token";

/// Install the test bearer-token env var. Idempotent — every primitive test
/// calls this before constructing the router.
fn ensure_test_token() {
    // SAFETY: every test sets the same value, so racing writers cannot
    // disagree. Mirrors the rationale in `integration_db.rs::ensure_test_token`.
    unsafe {
        std::env::set_var(API_TOKEN_ENV, TEST_BEARER);
    }
}

fn bearer_header() -> String {
    format!("Bearer {TEST_BEARER}")
}

async fn fresh_state() -> AppState {
    ensure_test_token();
    let state = AppState::new_sqlite_memory()
        .await
        .expect("sqlite memory state");
    state.migrate().await.expect("migrations apply");
    state
}

#[tokio::test]
async fn primitives_migration_creates_tables() {
    // 004_primitives.sql must land all three primitive tables alongside the
    // pre-existing component tables. Without this the rest of the suite
    // returns "no such table".
    let state = fresh_state().await;
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(state.pool().sqlite().expect("sqlite pool"))
            .await
            .unwrap();
    for required in ["symbols", "footprints", "sims"] {
        assert!(
            tables.iter().any(|t| t == required),
            "missing table {required}; have {tables:?}"
        );
    }
}

#[tokio::test]
async fn post_then_get_symbol_round_trip() {
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let mut sym = Symbol::empty("OPAMP-DUAL-8");
    // Replace the random uuid with a stable one we can assert on by URL.
    sym.uuid = Uuid::now_v7();

    let body = serde_json::json!({
        "library_id": library_id,
        "uuid": sym.uuid,
        "name": sym.name,
        "anchor": sym.anchor,
        "pins": sym.pins,
        "graphics": sym.graphics,
        "schematic_params": sym.schematic_params,
        "created": sym.created,
        "updated": sym.updated,
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/symbols")
                .header("content-type", "application/json")
                .header("authorization", bearer_header())
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/symbols/{}?library_id={}", sym.uuid, library_id))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let got: Symbol = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got.uuid, sym.uuid);
    assert_eq!(got.name, sym.name);
    assert_eq!(got.pins.len(), sym.pins.len());
}

#[tokio::test]
async fn list_symbols_filters_by_library_id() {
    // Two libraries, two symbols each. `?library_id=` must scope the result.
    let state = fresh_state().await;
    let app = router_with_state(state);

    let lib_a = Uuid::now_v7();
    let lib_b = Uuid::now_v7();

    for (lib, name) in [
        (lib_a, "RES-2T"),
        (lib_a, "CAP-2T"),
        (lib_b, "OPAMP-8"),
        (lib_b, "MCU-100"),
    ] {
        let mut sym = Symbol::empty(name);
        sym.uuid = Uuid::now_v7();
        let body = serde_json::json!({
            "library_id": lib,
            "uuid": sym.uuid,
            "name": sym.name,
            "anchor": sym.anchor,
            "pins": sym.pins,
            "graphics": sym.graphics,
            "schematic_params": sym.schematic_params,
            "created": sym.created,
            "updated": sym.updated,
        });
        let r = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/symbols")
                    .header("content-type", "application/json")
                    .header("authorization", bearer_header())
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::CREATED);
    }

    let r = app
        .oneshot(
            Request::builder()
                .uri(format!("/symbols?library_id={lib_a}"))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
    let got: Vec<PrimitiveSummary> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|s| s.library_id == lib_a));
}

#[tokio::test]
async fn post_then_get_footprint_round_trip() {
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let mut fp = Footprint::empty("SOIC-8");
    fp.uuid = Uuid::now_v7();

    // Footprint has many serde-default fields — embed it via flatten.
    let mut body = serde_json::to_value(&fp).unwrap();
    body.as_object_mut()
        .unwrap()
        .insert("library_id".into(), serde_json::json!(library_id));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/footprints")
                .header("content-type", "application/json")
                .header("authorization", bearer_header())
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/footprints/{}?library_id={}", fp.uuid, library_id))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let got: Footprint = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got.uuid, fp.uuid);
    assert_eq!(got.name, fp.name);
    // Body3D defaults round-trip cleanly.
    assert_eq!(got.body_3d, fp.body_3d);
}

#[tokio::test]
async fn post_then_get_sim_round_trip() {
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let mut sm = SimModel::empty("LM358", SimKind::Spice3);
    sm.uuid = Uuid::now_v7();
    sm.body = ".SUBCKT LM358 IN OUT VCC GND\n.ENDS".into();

    let mut body = serde_json::to_value(&sm).unwrap();
    body.as_object_mut()
        .unwrap()
        .insert("library_id".into(), serde_json::json!(library_id));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sims")
                .header("content-type", "application/json")
                .header("authorization", bearer_header())
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/sims/{}?library_id={}", sm.uuid, library_id))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let got: SimModel = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got.uuid, sm.uuid);
    assert_eq!(got.name, sm.name);
    assert_eq!(got.body, sm.body);
    assert_eq!(got.kind, sm.kind);
}

#[tokio::test]
async fn get_symbol_404_when_unknown() {
    // Round-trip the not-found path so future refactors don't regress the
    // `Option<…>` → 404 conversion.
    let state = fresh_state().await;
    let app = router_with_state(state);

    let library_id = Uuid::now_v7();
    let unknown = Uuid::now_v7();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/symbols/{unknown}?library_id={library_id}"))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_symbol_400_without_library_id() {
    // The route requires `?library_id=` to disambiguate primitives that share
    // a uuid across libraries. Missing it is a client bug, surfaced as 400.
    let state = fresh_state().await;
    let app = router_with_state(state);

    let unknown = Uuid::now_v7();
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/symbols/{unknown}"))
                .header("authorization", bearer_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
