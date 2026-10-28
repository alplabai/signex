//! Paging on the list routes (`limit` / `offset`): windows, the last page,
//! the default and the cap.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use signex_library::primitive::Symbol;
use signex_library_server::db::{AppState, MAX_PAGE_LIMIT, PrimitiveSummary};
use signex_library_server::router_with_state;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common::{TEST_BEARER, install_test_token};

async fn names(app: &axum::Router, query: &str) -> Vec<String> {
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/symbols?{query}"))
                .header("authorization", format!("Bearer {TEST_BEARER}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(r.into_body(), 1 << 22).await.unwrap();
    let got: Vec<PrimitiveSummary> = serde_json::from_slice(&bytes).unwrap();
    got.into_iter().map(|s| s.name).collect()
}

async fn app_with_symbols(library: Uuid, count: usize) -> axum::Router {
    install_test_token();
    let state = AppState::new_sqlite_memory().await.unwrap();
    state.migrate().await.unwrap();
    for i in 0..count {
        let mut sym = Symbol::empty(format!("S{i:04}"));
        sym.uuid = Uuid::now_v7();
        assert!(state.insert_symbol(library, &sym).await.unwrap());
    }
    router_with_state(state)
}

#[tokio::test]
async fn list_pages_cover_the_list_once_and_the_last_page_is_short() {
    let lib = Uuid::now_v7();
    let app = app_with_symbols(lib, 5).await;
    let page = |limit: u32, offset: u32| format!("library_id={lib}&limit={limit}&offset={offset}");

    assert_eq!(names(&app, &page(2, 0)).await, ["S0000", "S0001"]);
    assert_eq!(names(&app, &page(2, 2)).await, ["S0002", "S0003"]);
    assert_eq!(names(&app, &page(2, 4)).await, ["S0004"], "short last page");
    assert!(names(&app, &page(2, 6)).await.is_empty(), "past the end");
}

#[tokio::test]
async fn list_defaults_to_the_first_page_and_clamps_the_limit() {
    let lib = Uuid::now_v7();
    let total = MAX_PAGE_LIMIT as usize + 5;
    let app = app_with_symbols(lib, total).await;

    // No limit: the default page, which is smaller than the list.
    let default_page = names(&app, &format!("library_id={lib}")).await;
    assert!(!default_page.is_empty() && default_page.len() < total);
    // An oversized limit is capped at MAX_PAGE_LIMIT, not honoured.
    let capped = names(&app, &format!("library_id={lib}&limit=1000000")).await;
    assert_eq!(capped.len(), MAX_PAGE_LIMIT as usize);
    // Negative offset and zero limit are clamped, not errors.
    assert_eq!(
        names(&app, &format!("library_id={lib}&limit=0&offset=-3")).await,
        ["S0000"]
    );
    // The tail past the cap is reachable with an offset.
    let tail = names(
        &app,
        &format!("library_id={lib}&offset={MAX_PAGE_LIMIT}&limit={MAX_PAGE_LIMIT}"),
    )
    .await;
    assert_eq!(tail.len(), 5);
}
