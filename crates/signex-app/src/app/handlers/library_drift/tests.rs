//! #1903 — an unreadable library is reported once per (library, reason).

use std::path::PathBuf;

use uuid::Uuid;

use super::*;
use crate::diagnostics::logged_count;

fn report(library: Uuid, name: &str, reason: &str) -> DriftReport {
    let mut report = DriftReport::default();
    report
        .unreadable_libs
        .insert(library, (name.to_owned(), reason.to_owned()));
    report
}

fn mounted(id: Uuid) -> MountedLibrary {
    MountedLibrary {
        id,
        path: PathBuf::from("/tmp/x.snxlib"),
        display_name: "x".into(),
    }
}

fn app() -> Signex {
    Signex::new().0
}

#[test]
fn the_same_reason_is_logged_once_across_passes() {
    let mut app = app();
    let lib = Uuid::now_v7();
    let needle = "Library OnceLib1903";
    let before = logged_count(needle);
    let _ = app.handle_library_drift_checked(report(lib, "OnceLib1903", "disk gone"));
    let _ = app.handle_library_drift_checked(report(lib, "OnceLib1903", "disk gone"));
    assert_eq!(logged_count(needle) - before, 1);
}

#[test]
fn a_different_reason_on_the_same_library_is_logged_again() {
    let mut app = app();
    let lib = Uuid::now_v7();
    let needle = "Library ReasonLib1903";
    let before = logged_count(needle);
    let _ = app.handle_library_drift_checked(report(lib, "ReasonLib1903", "disk gone"));
    let _ = app.handle_library_drift_checked(report(lib, "ReasonLib1903", "permission denied"));
    assert_eq!(logged_count(needle) - before, 2);
}

#[test]
fn a_good_pass_in_between_logs_the_next_failure_again() {
    let mut app = app();
    let lib = Uuid::now_v7();
    let needle = "Library GoodLib1903";
    let before = logged_count(needle);
    let _ = app.handle_library_drift_checked(report(lib, "GoodLib1903", "disk gone"));
    let _ = app.handle_library_drift_checked(DriftReport::default());
    let _ = app.handle_library_drift_checked(report(lib, "GoodLib1903", "disk gone"));
    assert_eq!(logged_count(needle) - before, 2);
}

#[test]
fn a_worker_failure_keeps_the_unreadable_record() {
    let mut app = app();
    let lib = Uuid::now_v7();
    let _ = app.handle_library_drift_checked(report(lib, "WorkerLib1903", "disk gone"));
    let failed = DriftReport {
        problems: vec!["library drift worker failed".into()],
        ..Default::default()
    };
    let _ = app.handle_library_drift_checked(failed);
    assert!(app.library.drift.unreadable_libs.contains_key(&lib));
}

#[test]
fn a_remounted_unreadable_library_is_reported_again() {
    let mut app = app();
    let (lib, other) = (Uuid::now_v7(), Uuid::now_v7());
    let needle = "Library RemountLib1903";
    let before = logged_count(needle);
    let _ = app.handle_library_drift_checked(report(lib, "RemountLib1903", "disk gone"));

    // Unmounted: only `other` is open now.
    app.prune_unmounted_drift(&[mounted(other)]);
    assert!(app.library.drift.unreadable_libs.is_empty());

    let _ = app.handle_library_drift_checked(report(lib, "RemountLib1903", "disk gone"));
    assert_eq!(logged_count(needle) - before, 2);
}

/// A fallback body stored under a key that is not its own id ("Device:R" for
/// a body whose id is "R") is re-keyed on load to "R#hash"; drift still
/// judges it against the library symbol the *instance* names (#2209).
#[test]
fn a_re_keyed_body_is_judged_against_its_instances_lib_id() {
    use signex_types::schematic::{LibSymbol, SchematicSheet, Symbol};
    let body: LibSymbol = serde_json::from_str(r#"{"id":"R"}"#).unwrap();
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert("Device:R".into(), body);
    sheet.symbols.push(Symbol {
        lib_id: "Device:R".into(),
        ..Default::default()
    });
    sheet.upgrade_unversioned_bodies();
    assert!(sheet.lib_symbols.keys().all(|k| k.starts_with("R#")));

    let entries = referenced_bodies(&sheet);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].1, "Device:R");
}
