//! M11: wiremock-backed integration tests for `DatabaseAdapter`.
//!
//! Mirrors the `distributor_*` test layout — a private tokio runtime drives
//! a `MockServer`; the adapter (which uses `reqwest::blocking`) runs on a
//! standard thread so its blocking calls don't deadlock the runtime.
//!
//! These tests pin the **wire shape** the adapter assumes:
//! - `GET /components` returns `Vec<ComponentSummary>`
//! - `GET /components/<uuid>` returns `Component`
//! - `POST /components/<uuid>/revisions` accepts `Revision` JSON
//! - `POST /components/<uuid>/locks` returns 201 on grant, 409 on conflict
//!   with body `{"error": "lock held by <holder>"}`
//!
//! Without these, a server-side change to the 409 envelope would produce
//! `LibraryError::Locked { holder: "unknown" }` silently — exactly the
//! M11 hazard called out in the review.

#![cfg(feature = "database")]

use std::future::Future;

use serde_json::json;
use signex_library::adapter::{FieldSet, LibraryAdapter, LibraryQuery};
use signex_library::adapters::database::DatabaseAdapter;
use signex_library::component::{Component, Revision};
use signex_library::embed::{PcbSide, SchematicSide, SharedSide};
use signex_library::identity::{InternalPn, Version};
use signex_library::lifecycle::LifecycleState;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test bearer token — every request must carry it via `Authorization: Bearer`.
const TEST_TOKEN: &str = "wiremock-bearer-token";
const TEST_HOLDER: &str = "test@signex";

fn fixture_revision(version: Version) -> Revision {
    let mut rev = Revision {
        version,
        state: LifecycleState::Released,
        created: chrono::Utc::now(),
        author: "test".into(),
        message: "fixture".into(),
        schematic: SchematicSide::default(),
        pcb: PcbSide::default(),
        shared: SharedSide {
            mpn: format!("MPN-{version}"),
            manufacturer: "Acme".into(),
            description: format!("part {version}"),
            ..Default::default()
        },
        content_hash: [0u8; 32],
    };
    rev.refresh_content_hash();
    rev
}

fn fixture_component(uuid: Uuid) -> Component {
    Component {
        uuid,
        internal_pn: InternalPn::new("R0805_10k"),
        revisions: vec![fixture_revision(Version::new(1, 0))],
        head: Version::new(1, 0),
    }
}

/// Spin up a wiremock `MockServer` on a private runtime, run the setup
/// closure to register expectations, then hand the adapter (built against
/// the server URL) to the synchronous test body on a fresh thread.
///
/// The pattern matches `distributor_mouser.rs::with_mock_server`; the only
/// difference is the URL we hand to the test is the bare server origin so
/// the adapter can append its own paths.
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

#[test]
fn search_round_trips_through_get_components() {
    let uuid = Uuid::now_v7();
    let summary_body = json!([
        {
            "uuid": uuid,
            "internal_pn": "R0805_10k",
            "mpn": "RC0805FR-0710KL",
            "head": { "major": 1, "minor": 0 },
            "state": "Released",
            "description": "Thick film resistor"
        }
    ]);

    with_mock_server(
        |server| {
            let body = summary_body.clone();
            Box::pin(async move {
                Mock::given(method("GET"))
                    .and(path("/components"))
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
            let hits = adapter.search(&LibraryQuery::default()).expect("search");
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].uuid, uuid);
            assert_eq!(hits[0].internal_pn.as_str(), "R0805_10k");
        },
    );
}

#[test]
fn get_component_round_trips_through_get_components_uuid() {
    let uuid = Uuid::now_v7();
    let comp = fixture_component(uuid);
    let body = serde_json::to_value(&comp).unwrap();
    let path_str = format!("/components/{uuid}");

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
            let got = adapter.get_component(uuid).expect("get_component");
            assert_eq!(got.uuid, uuid);
            assert_eq!(got.internal_pn.as_str(), "R0805_10k");
            assert_eq!(got.revisions.len(), 1);
        },
    );
}

#[test]
fn save_revision_posts_to_components_uuid_revisions() {
    let uuid = Uuid::now_v7();
    let rev = fixture_revision(Version::new(1, 1));
    let path_str = format!("/components/{uuid}/revisions");

    with_mock_server(
        move |server| {
            let path_for_mock = path_str.clone();
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path(path_for_mock.as_str()))
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
            adapter
                .save_revision(uuid, rev, "v1.1 fixture")
                .expect("save_revision");
        },
    );
}

#[test]
fn try_lock_happy_path_returns_201() {
    let uuid = Uuid::now_v7();
    let path_str = format!("/components/{uuid}/locks");

    with_mock_server(
        move |server| {
            let path_for_mock = path_str.clone();
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path(path_for_mock.as_str()))
                    .and(header(
                        "authorization",
                        format!("Bearer {TEST_TOKEN}").as_str(),
                    ))
                    .and(header("x-signex-holder", TEST_HOLDER))
                    .respond_with(ResponseTemplate::new(201))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            adapter
                .try_lock(uuid, FieldSet::Symbol)
                .expect("try_lock acquires");
        },
    );
}

#[test]
fn try_lock_409_conflict_maps_to_library_error_locked() {
    let uuid = Uuid::now_v7();
    let path_str = format!("/components/{uuid}/locks");

    with_mock_server(
        move |server| {
            let path_for_mock = path_str.clone();
            Box::pin(async move {
                Mock::given(method("POST"))
                    .and(path(path_for_mock.as_str()))
                    .respond_with(ResponseTemplate::new(409).set_body_json(json!({
                        "error": "lock held by alice"
                    })))
                    .expect(1)
                    .mount(server)
                    .await;
            })
        },
        move |adapter| {
            let err = adapter.try_lock(uuid, FieldSet::Symbol).unwrap_err();
            // The current envelope shape is `{"error": "..."}` and the
            // adapter pulls the holder from there. Pinning the assertion
            // surfaces silent envelope drift via this very test.
            match err {
                signex_library::LibraryError::Locked { holder, field_set } => {
                    assert!(holder.contains("alice"), "holder string was {holder}");
                    assert_eq!(field_set, "Symbol");
                }
                other => panic!("expected LibraryError::Locked, got {other:?}"),
            }
        },
    );
}
