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
use signex_library::component::{ComponentRow, DatasheetDoc, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMeta, LibraryMode, Manifest};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::{Footprint, PrimitiveRef, SimKind, SimModel, Symbol};
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
        self.adapter_named(library_id, "remote")
    }

    /// A client for `library_id` whose library is called `name`.
    fn adapter_named(&self, library_id: Uuid, name: &str) -> DatabaseAdapter {
        DatabaseAdapter::new(Manifest {
            library: LibraryMeta {
                name: name.into(),
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

// ── Stale row writes (#115) ──────────────────────────────────────────────

fn test_row(pn: &str) -> ComponentRow {
    let lib = Uuid::now_v7();
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        datasheet_label: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib, Uuid::now_v7()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", format!("MPN-{pn}")),
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

/// Two users open the same row at revision 1. Alice saves first; Bob's
/// save, still based on revision 1, is refused and Alice's edit survives.
#[test]
fn a_save_based_on_a_stale_revision_is_refused() {
    let server = TestServer::start();
    let library = Uuid::now_v7();
    let (alice, bob) = (server.adapter(library), server.adapter(library));
    let row = test_row("R-STALE");
    let id = RowId::from_uuid(row.row_id);
    alice
        .insert_row("resistors", row, "add")
        .expect("insert_row");

    let mut alice_copy = alice.read_row("resistors", id).expect("alice reads");
    let mut bob_copy = bob.read_row("resistors", id).expect("bob reads");
    assert_eq!(alice_copy.server_revision, Some(1));
    assert_eq!(bob_copy.server_revision, Some(1));

    alice_copy.primary_mpn.mpn = "ALICE".into();
    let revision = alice
        .update_row("resistors", alice_copy, "alice edits")
        .expect("alice saves");
    assert_eq!(revision, Some(2));

    bob_copy.primary_mpn.mpn = "BOB".into();
    let err = bob
        .update_row("resistors", bob_copy, "bob edits")
        .expect_err("bob's base is stale");
    assert!(
        matches!(
            err,
            LibraryError::RowChanged {
                current_revision: 2
            }
        ),
        "{err:?}"
    );

    let stored = bob.read_row("resistors", id).expect("read back");
    assert_eq!(stored.primary_mpn.mpn, "ALICE");
    assert_eq!(stored.server_revision, Some(2));
}

/// A delete based on a stale revision is refused and the row stays; one
/// based on the current revision goes through.
#[test]
fn a_delete_based_on_a_stale_revision_is_refused() {
    let server = TestServer::start();
    let library = Uuid::now_v7();
    let (alice, bob) = (server.adapter(library), server.adapter(library));
    let row = test_row("R-DEL");
    let id = RowId::from_uuid(row.row_id);
    alice
        .insert_row("resistors", row, "add")
        .expect("insert_row");
    let copy = alice.read_row("resistors", id).expect("read");
    alice
        .update_row("resistors", copy, "edit")
        .expect("alice saves");

    let err = bob
        .delete_row("resistors", id, Some(1), "drop")
        .expect_err("bob's base is stale");
    assert!(
        matches!(
            err,
            LibraryError::RowChanged {
                current_revision: 2
            }
        ),
        "{err:?}"
    );
    assert!(bob.read_row("resistors", id).is_ok(), "the row is kept");

    bob.delete_row("resistors", id, Some(2), "drop")
        .expect("current base deletes");
    assert!(matches!(
        bob.read_row("resistors", id),
        Err(LibraryError::NotFound(_))
    ));
}

/// A library whose name is not ASCII can still update and delete rows.
#[test]
fn a_library_with_a_non_ascii_name_updates_and_deletes_rows() {
    let server = TestServer::start();
    let adapter = server.adapter_named(Uuid::now_v7(), "Dirençler");
    let row = test_row("R-UTF8");
    let id = RowId::from_uuid(row.row_id);
    adapter
        .insert_row("resistors", row, "add")
        .expect("insert_row");
    let mut copy = adapter.read_row("resistors", id).expect("read");
    copy.primary_mpn.mpn = "EDITED".into();
    let revision = adapter
        .update_row("resistors", copy, "edit")
        .expect("update_row");
    adapter
        .delete_row("resistors", id, revision, "drop")
        .expect("delete_row");
}

fn with_documents(mut row: ComponentRow) -> ComponentRow {
    row.set_documents(vec![
        DatasheetDoc::new("TI", DatasheetRef::url("https://ti.example/u1.pdf")),
        DatasheetDoc::new("Errata", DatasheetRef::hash_pinned("ab12", "errata.pdf")),
    ]);
    row
}

/// Labelled documents survive an insert, an update and a read through the
/// real server (#1231).
#[test]
fn documents_round_trip_through_the_real_server() {
    let server = TestServer::start();
    let adapter = server.adapter(Uuid::now_v7());
    let row = with_documents(test_row("U-DOCS"));
    let id = RowId::from_uuid(row.row_id);
    adapter
        .insert_row("ics", row.clone(), "add")
        .expect("insert_row");
    let mut read = adapter.read_row("ics", id).expect("read");
    assert_eq!(read.documents(), row.documents());

    read.more_datasheets.clear();
    adapter
        .update_row("ics", read, "drop the errata")
        .expect("a new client may remove documents");
    assert!(
        adapter
            .read_row("ics", id)
            .unwrap()
            .more_datasheets
            .is_empty()
    );
}

/// A client built before labelled documents sends a row without them,
/// declaring an older row schema or none. The server refuses the write, so
/// the stored documents survive (#1239 review, #1230 guard).
#[test]
fn an_older_clients_write_is_refused_and_keeps_the_stored_documents() {
    let server = TestServer::start();
    let library = Uuid::now_v7();
    let adapter = server.adapter(library);
    let row = with_documents(test_row("U-OLD"));
    let id = RowId::from_uuid(row.row_id);
    adapter
        .insert_row("ics", row.clone(), "add")
        .expect("insert_row");

    // What an older client sends: the row it read, minus the fields it
    // does not know, with an edit of its own.
    let mut body = serde_json::to_value(adapter.read_row("ics", id).unwrap()).unwrap();
    let object = body.as_object_mut().unwrap();
    object.remove("datasheet_label");
    object.remove("more_datasheets");
    object.insert("internal_pn".into(), "U-OLD-EDITED".into());
    let url = format!(
        "{}/tables/ics/rows/{}?library_id={library}",
        server.url, row.row_id
    );
    for schema in [None, Some(signex_library::ROW_SCHEMA - 1)] {
        let (status, reply) = server._runtime.block_on(async {
            let mut req = reqwest::Client::new()
                .put(&url)
                .header("if-match", "\"1\"")
                .header("x-signex-message", "old client edit");
            if let Some(n) = schema {
                req = req.header(signex_library::ROW_SCHEMA_HEADER, n.to_string());
            }
            let resp = req.json(&body).send().await.unwrap();
            let status = resp.status();
            let reply: serde_json::Value = resp.json().await.unwrap();
            (status, reply)
        });
        assert_eq!(
            status,
            reqwest::StatusCode::BAD_REQUEST,
            "schema {schema:?}"
        );
        // Only the schema guard may refuse it, not an unrelated 400.
        assert_eq!(
            reply["code"],
            signex_library::ROW_SCHEMA_REFUSED_CODE,
            "schema {schema:?}"
        );
    }

    let stored = adapter.read_row("ics", id).unwrap();
    assert_eq!(stored.internal_pn.as_str(), "U-OLD");
    assert_eq!(stored.documents(), row.documents());
}
