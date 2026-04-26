//! Integration tests for WS-B — DB schema migrations, components/revisions/locks
//! routes, lock contention, git-export round-trip, and database adapter client.
//!
//! Default backend: in-memory SQLite. Postgres path is gated behind
//! `SIGNEX_TEST_PG_URL` env var so CI without Postgres still passes.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use signex_library::adapter::{FieldSet, LibraryAdapter, LibraryQuery};
use signex_library::adapters::database::DatabaseAdapter;
use signex_library::component::{Component, Revision};
use signex_library::embed::{PcbSide, SchematicSide, SharedSide};
use signex_library::identity::{InternalPn, Version};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMeta, LibraryMode, Manifest};
use signex_library::snxpart::{read_snxpart, snxpart_filename};
use signex_library_server::db::AppState;
use signex_library_server::git_export::export_to_dir;
use signex_library_server::router_with_state;
use tower::ServiceExt;
use uuid::Uuid;

fn fixture_revision(version: Version) -> Revision {
    let mut rev = Revision {
        version,
        state: LifecycleState::Released,
        created: Utc::now(),
        author: "test@signex".into(),
        message: format!("rev {version}"),
        schematic: SchematicSide::default(),
        pcb: PcbSide::default(),
        shared: SharedSide {
            mpn: format!("MPN-{}", version),
            manufacturer: "Acme".into(),
            description: format!("part {version}"),
            ..Default::default()
        },
        content_hash: [0u8; 32],
    };
    rev.refresh_content_hash();
    rev
}

fn fixture_component() -> Component {
    Component {
        uuid: Uuid::now_v7(),
        internal_pn: InternalPn::new("R0805_10k"),
        revisions: vec![fixture_revision(Version::new(1, 0))],
        head: Version::new(1, 0),
    }
}

async fn fresh_state() -> AppState {
    let state = AppState::new_sqlite_memory()
        .await
        .expect("sqlite memory state");
    state.migrate().await.expect("migrations apply");
    state
}

#[tokio::test]
async fn migrations_apply_cleanly() {
    let state = fresh_state().await;
    // After migrations, the seven tables must exist.
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(state.pool().sqlite().expect("sqlite pool"))
            .await
            .unwrap();

    for required in [
        "components",
        "revisions",
        "parameters",
        "suppliers",
        "lifecycle_log",
        "locks",
        "review_requests",
    ] {
        assert!(
            tables.iter().any(|t| t == required),
            "missing table {required}; have {tables:?}"
        );
    }
}

#[tokio::test]
async fn post_then_get_component_round_trip() {
    let state = fresh_state().await;
    let app = router_with_state(state);

    let comp = fixture_component();
    let body = serde_json::to_vec(&comp).unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/components")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/components/{}", comp.uuid))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let got: Component = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(got.uuid, comp.uuid);
    assert_eq!(got.head, comp.head);
    assert_eq!(got.revisions.len(), 1);
    assert_eq!(
        got.revisions[0].content_hash,
        comp.revisions[0].content_hash
    );
}

#[tokio::test]
async fn lock_contention_second_attempt_blocks_until_release() {
    let state = fresh_state().await;
    // Use a short TTL so the test runs fast.
    state.locks().set_idle_ttl(Duration::from_millis(200));

    let comp_uuid = Uuid::now_v7();

    // First holder grabs the lock.
    state
        .locks()
        .try_lock(comp_uuid, FieldSet::Symbol, "alice")
        .expect("alice acquires");

    // Second holder fails immediately.
    let err = state
        .locks()
        .try_lock(comp_uuid, FieldSet::Symbol, "bob")
        .unwrap_err();
    assert!(matches!(
        err.kind,
        signex_library_server::locks::LockErrorKind::Held { .. }
    ));

    // Release, then bob succeeds.
    state
        .locks()
        .release(comp_uuid, FieldSet::Symbol, "alice")
        .unwrap();
    state
        .locks()
        .try_lock(comp_uuid, FieldSet::Symbol, "bob")
        .expect("bob acquires after release");

    // Different field set is independently lockable.
    state
        .locks()
        .try_lock(comp_uuid, FieldSet::Footprint, "alice")
        .expect("different field-set is independent");
}

#[tokio::test]
async fn lock_contention_ttl_expiry_allows_takeover() {
    let state = fresh_state().await;
    state.locks().set_idle_ttl(Duration::from_millis(50));

    let comp_uuid = Uuid::now_v7();
    state
        .locks()
        .try_lock(comp_uuid, FieldSet::Symbol, "alice")
        .unwrap();

    // Wait past TTL.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Bob can now claim — alice's lock has expired.
    state
        .locks()
        .try_lock(comp_uuid, FieldSet::Symbol, "bob")
        .expect("bob takes over after TTL");
}

#[tokio::test]
async fn git_export_round_trip_three_components() {
    let state = fresh_state().await;
    let mut comps = Vec::new();
    for i in 0..3 {
        let mut c = fixture_component();
        c.internal_pn = InternalPn::new(format!("PART_{i}"));
        state.insert_component(&c).await.unwrap();
        comps.push(c);
    }

    let dir = tempfile::tempdir().unwrap();
    export_to_dir(&state, dir.path()).await.unwrap();

    // Assert .snxpart files exist in <uuid>/<uuid>-1.0.snxpart layout.
    for c in &comps {
        let part_path = dir
            .path()
            .join(c.uuid.to_string())
            .join(snxpart_filename(c.uuid, c.revisions[0].version));
        assert!(part_path.exists(), "expected {part_path:?}");
        let part = read_snxpart(&part_path).unwrap();
        assert_eq!(part.uuid, c.uuid);
        assert_eq!(part.internal_pn, c.internal_pn);
        assert_eq!(part.revision.content_hash, c.revisions[0].content_hash);
    }

    // Manifest at the export root.
    let manifest_path = dir.path().join("manifest.toml");
    assert!(manifest_path.exists());
    let mtext = std::fs::read_to_string(&manifest_path).unwrap();
    let m = Manifest::parse(&mtext).unwrap();
    assert!(matches!(m.mode, LibraryMode::LocalGit));
    assert_eq!(m.library.name, "exported-library");
}

/// Spin up the server in-process, point a `DatabaseAdapter` at it, exercise
/// the full LibraryAdapter trait surface. Adapter is blocking (uses reqwest's
/// blocking client) so it must run on a non-async thread; we hand it to
/// `tokio::task::spawn_blocking`.
#[tokio::test(flavor = "multi_thread")]
async fn database_adapter_round_trips_through_http() {
    let state = fresh_state().await;
    let app = router_with_state(state);

    // Bind to an OS-assigned port so parallel tests don't collide.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let manifest = Manifest {
        library: LibraryMeta {
            name: "test".into(),
            library_id: Uuid::now_v7(),
            description: None,
        },
        mode: LibraryMode::Database {
            url: format!("http://{addr}"),
            auth: "test-token".into(),
        },
        workflow: Default::default(),
        users: Default::default(),
    };

    let comp = fixture_component();
    let comp_clone = comp.clone();
    let manifest_clone = manifest.clone();
    tokio::task::spawn_blocking(move || {
        let adapter = Arc::new(DatabaseAdapter::new(manifest_clone).expect("adapter constructs"));
        adapter
            .save_revision(comp_clone.uuid, comp_clone.revisions[0].clone(), "initial")
            .expect("first save");

        let summaries = adapter.search(&LibraryQuery::default()).expect("search");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].uuid, comp_clone.uuid);

        adapter
            .try_lock(comp_clone.uuid, FieldSet::Symbol)
            .expect("first lock");
        adapter
            .release_lock(comp_clone.uuid, FieldSet::Symbol)
            .expect("release");
    })
    .await
    .expect("spawn_blocking completed");

    let _ = comp;
    let _ = manifest;
}

#[tokio::test]
async fn locks_endpoint_returns_409_when_held() {
    let state = fresh_state().await;
    state.locks().set_idle_ttl(Duration::from_secs(60));
    let app = router_with_state(state);

    let comp_uuid = Uuid::now_v7();

    let mk_req = |holder: &str| {
        Request::builder()
            .method("POST")
            .uri(format!("/components/{comp_uuid}/locks"))
            .header("content-type", "application/json")
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
