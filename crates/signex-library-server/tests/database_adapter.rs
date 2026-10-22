//! `DatabaseAdapter` against the real server router (#688).
//!
//! The client's own tests in signex-library use wiremock stubs, which only
//! prove the client matches the stubs. These tests serve the real router on
//! a loopback port, backed by in-memory SQLite, and drive the real client
//! against it, so a contract mismatch on either side fails here.
//!
//! The tests never write the environment: the server takes its token from
//! `SIGNEX_API_TOKEN` as the process started, and the client sends the same
//! value. Unset, the loopback server runs unauthenticated; the wiremock tests
//! in signex-library cover the auth header.

use signex_library::adapter::{LibraryAdapter, LibraryError, PrimitiveSummary};
use signex_library::adapters::database::DatabaseAdapter;
use signex_library::manifest::{LibraryMeta, LibraryMode, Manifest};
use signex_library::primitive::{Footprint, SimKind, SimModel, Symbol};
use signex_library_server::db::AppState;
use signex_library_server::{API_TOKEN_ENV, router_with_state};
use uuid::Uuid;

/// A real server on a loopback port. The runtime lives as long as the value.
struct TestServer {
    url: String,
    _runtime: tokio::runtime::Runtime,
}

impl TestServer {
    fn start() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("runtime");
        let url = runtime.block_on(async {
            let state = AppState::new_sqlite_memory()
                .await
                .expect("sqlite memory state");
            state.migrate().await.expect("migrations apply");
            let router = router_with_state(state);
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind loopback");
            let url = format!("http://{}", listener.local_addr().expect("local addr"));
            tokio::spawn(async move {
                axum::serve(listener, router).await.expect("serve");
            });
            url
        });
        Self {
            url,
            _runtime: runtime,
        }
    }

    /// A client for the library `library_id` on this server.
    fn adapter(&self, library_id: Uuid) -> DatabaseAdapter {
        DatabaseAdapter::new(Manifest {
            library: LibraryMeta {
                name: "remote".into(),
                library_id,
                description: None,
            },
            mode: LibraryMode::Database {
                url: self.url.clone(),
                auth: std::env::var(API_TOKEN_ENV).unwrap_or_default(),
            },
            workflow: Default::default(),
            users: Default::default(),
            tables: Vec::new(),
        })
        .expect("adapter")
    }
}

fn uuids(list: Vec<PrimitiveSummary>) -> Vec<Uuid> {
    list.into_iter().map(|summary| summary.uuid).collect()
}

#[test]
fn primitives_save_get_and_list_through_the_real_server() {
    let server = TestServer::start();
    let adapter = server.adapter(Uuid::now_v7());

    let symbol = Symbol::empty("OPAMP-DUAL-8");
    let footprint = Footprint::empty("SOIC-8");
    let model = SimModel::empty("LM358", SimKind::Spice3);

    adapter
        .save_symbol(symbol.clone(), "add symbol")
        .expect("save_symbol");
    adapter
        .save_footprint(footprint.clone(), "add footprint")
        .expect("save_footprint");
    adapter
        .save_sim(model.clone(), "add sim")
        .expect("save_sim");

    assert_eq!(
        adapter.get_symbol(symbol.uuid).expect("get_symbol").name,
        "OPAMP-DUAL-8"
    );
    assert_eq!(
        adapter
            .get_footprint(footprint.uuid)
            .expect("get_footprint")
            .name,
        "SOIC-8"
    );
    assert_eq!(adapter.get_sim(model.uuid).expect("get_sim").name, "LM358");

    assert_eq!(
        uuids(adapter.list_symbols().expect("list_symbols")),
        vec![symbol.uuid]
    );
    assert_eq!(
        uuids(adapter.list_footprints().expect("list_footprints")),
        vec![footprint.uuid]
    );
    assert_eq!(
        uuids(adapter.list_sims().expect("list_sims")),
        vec![model.uuid]
    );
}

#[test]
fn primitives_stay_in_their_own_library() {
    let server = TestServer::start();
    let library_a = server.adapter(Uuid::now_v7());
    let library_b = server.adapter(Uuid::now_v7());

    let symbol_a = Symbol::empty("IN-A");
    let symbol_b = Symbol::empty("IN-B");
    library_a
        .save_symbol(symbol_a.clone(), "add a")
        .expect("save in A");
    library_b
        .save_symbol(symbol_b.clone(), "add b")
        .expect("save in B");

    assert_eq!(
        uuids(library_a.list_symbols().expect("list A")),
        vec![symbol_a.uuid]
    );
    assert_eq!(
        uuids(library_b.list_symbols().expect("list B")),
        vec![symbol_b.uuid]
    );
    assert!(matches!(
        library_a.get_symbol(symbol_b.uuid),
        Err(LibraryError::NotFound(_))
    ));
}
