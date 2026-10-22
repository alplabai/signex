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

    let (ctx, issues) = build_export_scope(&app.document_state).expect("scope");
    let refusal = issues
        .shared_reference_refusal(&ctx)
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

/// Opens `top` (placing `children`) plus each `(name, children)` below it, and
/// a `leaf` file holding one part with designator `leaf_ref`.
fn project_with_leaf(
    dir: &str,
    top_children: &[&str],
    middles: &[&str],
    leaf: &str,
    leaf_ref: &str,
    power: bool,
) -> crate::app::Signex {
    let mut app = app_workspace(dir, &["top.snxsch"]);
    let path = |name: &str| PathBuf::from(dir).join(name);
    open_with(
        &mut app.document_state,
        &path("top.snxsch"),
        sheet_with_net("R_TOP", "TOP_NET", top_children),
    );
    for middle in middles {
        open_with(
            &mut app.document_state,
            &path(middle),
            sheet_with_net(&format!("R_{middle}"), "MID_NET", &[leaf]),
        );
    }
    let mut leaf_sheet = sheet_with_net(leaf_ref, "LEAF_NET", &[]);
    if power {
        leaf_sheet.symbols[0].is_power = true;
    }
    open_with(&mut app.document_state, &path(leaf), leaf_sheet);
    app.document_state.active_path = Some(path("top.snxsch"));
    app
}

fn refusal_of(app: &crate::app::Signex) -> Option<String> {
    let (ctx, issues) = build_export_scope(&app.document_state).expect("scope");
    issues.shared_reference_refusal(&ctx)
}

#[test]
fn a_child_holding_only_power_ports_placed_twice_is_not_refused() {
    for leaf_ref in ["#PWR01", "GND1"] {
        let dir = format!("/w/pwr-{leaf_ref}");
        let mut app = project_with_leaf(
            &dir,
            &["child.snxsch", "child.snxsch"],
            &[],
            "child.snxsch",
            leaf_ref,
            leaf_ref == "GND1",
        );
        assert_eq!(refusal_of(&app), None, "{leaf_ref}");

        let out = std::env::temp_dir().join(format!("signex-protel-pwr-{}.NET", Uuid::new_v4()));
        let _ = app.handle_export_netlist_finished(Ok(out.clone()));
        assert!(app.document_state.error_notice.is_none(), "{leaf_ref}");
        assert!(out.exists(), "{leaf_ref}: a power-only child still exports");
        std::fs::remove_file(&out).ok();
    }
}

#[test]
fn power_references_are_not_listed_beside_a_real_shared_part() {
    let shared = |reference: &str| signex_net::StitchIssue::SharedReferenceAcrossInstances {
        filename: "child.snxsch".to_string(),
        reference: reference.to_string(),
    };
    let issues = crate::app::handlers::menu::export::ExportIssues {
        stitch: vec![shared("#PWR01"), shared("R1")],
        ..Default::default()
    };
    let ctx = signex_output::ExportContext {
        sheets: Vec::new(),
        metadata: Default::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
    };
    let refusal = issues.shared_reference_refusal(&ctx).expect("R1 is shared");
    assert!(refusal.contains("R1"), "{refusal}");
    assert!(!refusal.contains("#PWR"), "{refusal}");
}

#[test]
fn a_child_placed_by_two_different_parents_is_refused_and_nothing_is_written() {
    let mut app = project_with_leaf(
        "/w/two",
        &["a.snxsch", "b.snxsch"],
        &["a.snxsch", "b.snxsch"],
        "child.snxsch",
        "R1",
        false,
    );
    let refusal = refusal_of(&app).expect("two parents sharing one child must be refused");
    assert!(
        refusal.contains("R1") && refusal.contains("child.snxsch"),
        "{refusal}"
    );

    let out = std::env::temp_dir().join(format!("signex-protel-parents-{}.NET", Uuid::new_v4()));
    let _ = app.handle_export_netlist_finished(Ok(out.clone()));
    assert!(!out.exists(), "a refused export must not write a file");
    assert!(app.document_state.error_notice.is_some());
}
