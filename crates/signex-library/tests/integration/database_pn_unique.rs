//! The database backend refuses a part number another row holds, like the
//! local git backend: compared trimmed and case-sensitively, a blank PN
//! never conflicts, and rows that already clash stay editable.

#![cfg(feature = "database")]

use signex_library::adapter::{LibraryAdapter, LibraryError};
use signex_library::component::{ROW_SCHEMA, ROW_SCHEMA_HEADER};
use signex_library::identity::InternalPn;
use uuid::Uuid;
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, ResponseTemplate};

use super::database_adapter::{mk_row, with_mock_server};

/// Serve `existing` as the one table `resistors`; a write answers 200 with
/// the row it was sent, so only the adapter's own refusal can fail a write.
fn serve(
    existing: Vec<signex_library::component::ComponentRow>,
) -> impl FnOnce(&wiremock::MockServer) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_>>
{
    move |server| {
        Box::pin(async move {
            Mock::given(method("GET"))
                .and(path("/tables"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(serde_json::json!(["resistors"])),
                )
                .mount(server)
                .await;
            Mock::given(method("GET"))
                .and(path("/tables/resistors"))
                .respond_with(ResponseTemplate::new(200).set_body_json(&existing))
                .mount(server)
                .await;
            for row in &existing {
                Mock::given(method("GET"))
                    .and(path(format!("/tables/resistors/rows/{}", row.row_id)))
                    .respond_with(ResponseTemplate::new(200).set_body_json(row))
                    .mount(server)
                    .await;
                let mut stored = row.clone();
                stored.server_revision = Some(2);
                Mock::given(method("PUT"))
                    .and(path(format!("/tables/resistors/rows/{}", row.row_id)))
                    .and(header_exists(ROW_SCHEMA_HEADER))
                    .respond_with(ResponseTemplate::new(200).set_body_json(&stored))
                    .mount(server)
                    .await;
            }
            let _ = ROW_SCHEMA;
            Mock::given(method("POST"))
                .and(path("/tables/resistors/rows"))
                .respond_with(|req: &wiremock::Request| {
                    ResponseTemplate::new(201).set_body_bytes(req.body.clone())
                })
                .mount(server)
                .await;
        })
    }
}

#[test]
fn insert_refuses_a_part_number_another_row_holds() {
    let existing = mk_row("PN-A", "resistor");
    with_mock_server(serve(vec![existing]), |adapter| {
        let exact = adapter.insert_row("resistors", mk_row("PN-A", "resistor"), "m");
        assert!(matches!(exact, Err(LibraryError::Conflict(_))), "{exact:?}");
        let padded = adapter.insert_row("resistors", mk_row("  PN-A ", "resistor"), "m");
        assert!(
            matches!(padded, Err(LibraryError::Conflict(_))),
            "{padded:?}"
        );
        // Case-sensitive, and a blank draft PN never conflicts.
        adapter
            .insert_row("resistors", mk_row("pn-a", "resistor"), "m")
            .unwrap();
        adapter
            .insert_row("resistors", mk_row("", "resistor"), "m")
            .unwrap();
        adapter
            .insert_row("resistors", mk_row("PN-B", "resistor"), "m")
            .unwrap();
    });
}

#[test]
fn update_refuses_moving_onto_a_held_part_number_but_not_a_legacy_duplicate_edit() {
    let a = mk_row("DUP", "resistor");
    let mut dup = mk_row("DUP", "resistor");
    dup.row_id = Uuid::now_v7();
    let other = mk_row("PN-C", "resistor");
    with_mock_server(serve(vec![a, dup.clone(), other.clone()]), move |adapter| {
        // A row that already clashes keeps its other fields editable.
        let mut edit = dup.clone();
        edit.description = "edited".into();
        adapter.update_row("resistors", edit, "m").unwrap();
        // Moving another row onto the duplicated PN is refused.
        let mut clash = other.clone();
        clash.internal_pn = InternalPn::new("DUP ");
        let err = adapter.update_row("resistors", clash, "m");
        assert!(matches!(err, Err(LibraryError::Conflict(_))), "{err:?}");
    });
}
