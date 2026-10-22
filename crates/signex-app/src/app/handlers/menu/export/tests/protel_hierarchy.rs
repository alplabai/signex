//! The Protel `.NET` deliverable on a hierarchical project (#354), driven from
//! the real export scope rather than a hand-built context: every designator a
//! net block names must have a component block, even when it lives on a child
//! sheet that is not a printed page, and a child placed twice is refused.
//!
//! Split out of the parent module only to keep that file under the size cap.

use std::path::PathBuf;

use signex_output::{Exporter, NetlistExporter, NetlistOptions};
use uuid::Uuid;

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::handlers::menu::export::build_export_scope;

/// Designators declared by `[ ... ]` blocks and designators named by
/// `Designator-Pin` lines inside `( ... )` blocks.
fn declared_and_referenced(text: &str) -> (Vec<String>, Vec<String>) {
    let (mut declared, mut referenced) = (Vec::new(), Vec::new());
    let mut lines = text.split("\r\n");
    while let Some(line) = lines.next() {
        match line {
            "[" => declared.extend(lines.next().map(str::to_string)),
            "(" => {
                lines.next(); // net name
                for l in lines.by_ref().take_while(|l| *l != ")") {
                    if let Some((designator, _pin)) = l.rsplit_once('-') {
                        referenced.push(designator.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    (declared, referenced)
}

fn hierarchical_app() -> crate::app::Signex {
    let mut app = app_workspace("/w/h", &["top.snxsch"]);
    let top = PathBuf::from("/w/h").join("top.snxsch");
    let child = PathBuf::from("/w/h").join("child.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]),
    );
    open_with(
        &mut app.document_state,
        &child,
        sheet_with_net("R_CHILD", "CHILD_NET", &[]),
    );
    app.document_state.active_path = Some(top);
    app
}

#[test]
fn every_designator_in_a_net_block_has_a_component_block_on_a_hierarchy() {
    let app = hierarchical_app();
    let (ctx, issues) = build_export_scope(&app.document_state).expect("scope");
    assert!(issues.messages().is_empty(), "{:?}", issues.messages());

    let out = NetlistExporter
        .export(&ctx, &NetlistOptions::default())
        .expect("a complete hierarchy exports");
    let text = String::from_utf8(out.bytes).unwrap();
    let (declared, referenced) = declared_and_referenced(&text);

    assert!(referenced.contains(&"R_CHILD".to_string()), "{text}");
    for designator in &referenced {
        assert!(
            declared.contains(designator),
            "{designator} is in a net block with no component block:\n{text}"
        );
    }
}

#[test]
fn a_child_placed_twice_is_refused_and_nothing_is_written() {
    let mut app = app_workspace("/w/t", &["top.snxsch"]);
    let top = PathBuf::from("/w/t").join("top.snxsch");
    let child = PathBuf::from("/w/t").join("child.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch", "child.snxsch"]),
    );
    open_with(
        &mut app.document_state,
        &child,
        sheet_with_net("R1", "CHILD_NET", &[]),
    );
    app.document_state.active_path = Some(top);

    let (_ctx, issues) = build_export_scope(&app.document_state).expect("scope");
    let refusal = issues
        .shared_reference_refusal()
        .expect("the shared designator must be a refusal for the Protel file");
    assert!(
        refusal.contains("R1") && refusal.contains("child.snxsch"),
        "{refusal}"
    );

    let out = std::env::temp_dir().join(format!("signex-protel-twice-{}.NET", Uuid::new_v4()));
    let _ = app.handle_export_netlist_finished(Ok(out.clone()));
    assert!(!out.exists(), "a refused export must not write a file");
    assert!(app.document_state.error_notice.is_some());
}
