//! A cascade on the database backend updates its rows with ordered PUTs
//! (the server has no batch endpoint) and stops at the first failure
//! (#1362). The failure is in the report, not an `Err`.

#![cfg(feature = "database")]

use std::sync::{Arc, Mutex};

use signex_library::cascade::{CascadeKind, PrimitiveSave, cascade_after_saves};
use signex_library::component::{ComponentRow, ROW_SCHEMA_HEADER};
use signex_library::identity::RowId;
use signex_library::manifest::WorkflowMode;
use signex_library::primitive::PrimitiveRef;
use uuid::Uuid;
use wiremock::matchers::{header_exists, method, path};
use wiremock::{Mock, ResponseTemplate};

use super::database_adapter::{mk_row, with_mock_server};

fn serve(
    rows: Vec<ComponentRow>,
    fail_on: Option<Uuid>,
    puts: Arc<Mutex<Vec<Uuid>>>,
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
                .respond_with(ResponseTemplate::new(200).set_body_json(&rows))
                .mount(server)
                .await;
            for row in &rows {
                let id = row.row_id;
                Mock::given(method("GET"))
                    .and(path(format!("/tables/resistors/rows/{id}")))
                    .respond_with(ResponseTemplate::new(200).set_body_json(row))
                    .mount(server)
                    .await;
                let puts = Arc::clone(&puts);
                Mock::given(method("PUT"))
                    .and(path(format!("/tables/resistors/rows/{id}")))
                    .and(header_exists(ROW_SCHEMA_HEADER))
                    .respond_with(move |req: &wiremock::Request| {
                        puts.lock().unwrap().push(id);
                        if Some(id) == fail_on {
                            return ResponseTemplate::new(500).set_body_string("boom");
                        }
                        let mut stored: ComponentRow = serde_json::from_slice(&req.body).unwrap();
                        stored.server_revision = Some(2);
                        ResponseTemplate::new(200).set_body_json(&stored)
                    })
                    .mount(server)
                    .await;
            }
        })
    }
}

fn bound_rows(n: usize, lib: Uuid, sym: Uuid) -> Vec<ComponentRow> {
    (0..n)
        .map(|i| {
            let mut r = mk_row(&format!("PN-{i}"), "resistor");
            r.symbol_ref = PrimitiveRef::new(lib, sym);
            r.symbol_version = "0.0.1".into();
            r
        })
        .collect()
}

fn save(lib: Uuid, sym: Uuid) -> PrimitiveSave {
    PrimitiveSave {
        kind: CascadeKind::Symbol,
        library_id: lib,
        uuid: sym,
        version: "0.0.2".into(),
    }
}

#[test]
fn rows_are_put_in_order_and_each_bumps() {
    let (lib, sym) = (Uuid::now_v7(), Uuid::now_v7());
    let rows = bound_rows(3, lib, sym);
    let order: Vec<Uuid> = rows.iter().map(|r| r.row_id).collect();
    let puts = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&puts);
    let expected = order.clone();
    with_mock_server(serve(rows, None, puts), move |adapter| {
        let report =
            cascade_after_saves(&adapter, &[save(lib, sym)], WorkflowMode::Personal).unwrap();
        let bumped: Vec<Uuid> = report.auto_bumped.iter().map(|r| r.as_uuid()).collect();
        assert_eq!(bumped, expected);
        assert!(report.failed.is_empty(), "{:?}", report.failed);
    });
    assert_eq!(*seen.lock().unwrap(), order, "one PUT per row, in order");
}

#[test]
fn the_first_failure_stops_the_batch_and_is_reported() {
    let (lib, sym) = (Uuid::now_v7(), Uuid::now_v7());
    let rows = bound_rows(4, lib, sym);
    let order: Vec<Uuid> = rows.iter().map(|r| r.row_id).collect();
    let puts = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&puts);
    let expected = order.clone();
    with_mock_server(serve(rows, Some(order[1]), puts), move |adapter| {
        let report = cascade_after_saves(&adapter, &[save(lib, sym)], WorkflowMode::Personal)
            .expect("a failed row is in the report, not an Err");
        assert_eq!(report.auto_bumped, vec![RowId::from_uuid(expected[0])]);
        let failed: Vec<Uuid> = report.failed.iter().map(|(id, _)| id.as_uuid()).collect();
        assert_eq!(
            failed,
            expected[1..],
            "the failure and the rows not attempted"
        );
        assert!(report.failed[1].1.contains("not attempted"));
        assert!(
            report.permanent.is_empty(),
            "rows never attempted after a retryable failure are retryable: {:?}",
            report.permanent
        );
    });
    assert_eq!(
        *seen.lock().unwrap(),
        order[..2],
        "no PUT after the failing one"
    );
}
