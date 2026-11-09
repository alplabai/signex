//! #2209: Annotate and placement warn when units of one part end up on
//! different versions of their library symbol, once and only for the part
//! they touched.

use super::annotate_tests::{project, unit};
use crate::app::Signex;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const V1: &str = "Amplifier:LM358#00000001";
const V2: &str = "Amplifier:LM358#00000002";

fn on(mut sym: signex_types::schematic::Symbol, body: &str) -> signex_types::schematic::Symbol {
    sym.body_ref = body.to_owned();
    sym
}

/// Warnings that name `dir` (a unique temp directory) and `reference`.
fn warnings(dir: &std::path::Path, reference: &str) -> usize {
    let _ = crate::diagnostics::init_logging();
    let dir = dir.to_string_lossy().to_string();
    crate::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.contains(&dir) && e.message.starts_with(&format!("{reference} on ")))
        .count()
}

fn annotate(app: &mut Signex) {
    let _ = crate::diagnostics::init_logging();
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    crate::app::path_ops::settle_task(app, task);
}

#[test]
fn annotate_joining_two_versions_into_one_part_warns_once() {
    let (mut app, dir) = project(
        vec![on(unit("U?", 1, 0.0), V1), on(unit("U?", 2, 10.0), V2)],
        vec![],
    );
    annotate(&mut app);
    assert_eq!(warnings(&dir, "U1"), 1);
}

#[test]
fn unrelated_unannotated_units_on_different_versions_do_not_warn() {
    let (mut app, dir) = project(
        vec![on(unit("U?", 1, 0.0), V1), on(unit("U?", 1, 10.0), V2)],
        vec![],
    );
    annotate(&mut app);
    assert_eq!(warnings(&dir, "U1") + warnings(&dir, "U2"), 0);
}

#[test]
fn placing_an_unrelated_part_does_not_report_an_old_mixed_part_again() {
    let _ = crate::diagnostics::init_logging();
    let (app, dir) = project(
        vec![
            on(unit("U1", 1, 0.0), V1),
            on(unit("U1", 2, 10.0), V2),
            on(unit("R9", 1, 20.0), V1),
        ],
        vec![],
    );
    let path = dir.join("a_root.snxsch");
    let doc = app.document_state.engines[&path].document();
    let placed: HashSet<Uuid> = [doc.symbols[2].uuid].into();
    app.warn_mixed_version_parts(&HashMap::from([(path.clone(), placed)]));
    assert_eq!(warnings(&dir, "U1"), 0, "U1 was not touched");

    let joined: HashSet<Uuid> = [doc.symbols[1].uuid].into();
    app.warn_mixed_version_parts(&HashMap::from([(path, joined)]));
    assert_eq!(warnings(&dir, "U1"), 1);
}
