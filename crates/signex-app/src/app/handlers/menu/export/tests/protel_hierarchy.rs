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

    let out = std::env::temp_dir().join(format!("signex-protel-twice-{}.NET", Uuid::now_v7()));
    let task = app.handle_export_netlist_finished(
        app.document_state.active_path.clone().expect("active"),
        Ok(out.clone()),
    );
    crate::app::handlers::menu::export::settle(&mut app, task);
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

        let out = std::env::temp_dir().join(format!("signex-protel-pwr-{}.NET", Uuid::now_v7()));
        let task = app.handle_export_netlist_finished(
            app.document_state.active_path.clone().expect("active"),
            Ok(out.clone()),
        );
        crate::app::handlers::menu::export::settle(&mut app, task);
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
        current_sheet: None,
        netlist_placements: Vec::new(),
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

    let out = std::env::temp_dir().join(format!("signex-protel-parents-{}.NET", Uuid::now_v7()));
    let task = app.handle_export_netlist_finished(
        app.document_state.active_path.clone().expect("active"),
        Ok(out.clone()),
    );
    crate::app::handlers::menu::export::settle(&mut app, task);
    assert!(!out.exists(), "a refused export must not write a file");
    assert!(app.document_state.error_notice.is_some());
}

fn entry_for(chain: &[u128], reference: &str) -> signex_types::schematic::SymbolInstance {
    signex_types::schematic::SymbolInstance {
        project: "p".to_string(),
        path: signex_types::schematic::InstancePath::from_chain(
            chain.iter().map(|n| Uuid::from_u128(*n)).collect(),
        )
        .to_string(),
        reference: reference.to_string(),
        unit: 1,
    }
}

/// `top` placing `child.snxsch` as one sheet symbol per name in `places`
/// (uuids 1, 2, ...); the child holds `R1` on a local net with `entries`.
/// `listed` are the project's declared (printed) pages.
pub(super) fn placed_app(
    dir: &str,
    listed: &[&str],
    places: &[&str],
    entries: Vec<signex_types::schematic::SymbolInstance>,
) -> crate::app::Signex {
    use signex_types::schematic::LabelType;
    let mut app = app_workspace(dir, listed);
    let path = |name: &str| PathBuf::from(dir).join(name);
    let files = vec!["child.snxsch"; places.len()];
    let mut top = sheet_with_net("R_TOP", "TOP_NET", &files);
    for (n, (cs, name)) in top.child_sheets.iter_mut().zip(places).enumerate() {
        cs.name = name.to_string();
        cs.uuid = Uuid::from_u128(n as u128 + 1);
    }
    open_with(&mut app.document_state, &path("top.snxsch"), top);

    let mut child = sheet_with_net("R1", "CHILD_NET", &[]);
    child.labels[0].label_type = LabelType::Net;
    child.symbols[0].ref_text = Some(signex_types::schematic::TextProp {
        position: signex_types::schematic::Point::new(0.0, 2.0),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: Default::default(),
        justify_v: Default::default(),
        hidden: false,
        font_family: None,
    });
    child.symbols[0].instances = entries;
    open_with(&mut app.document_state, &path("child.snxsch"), child);
    app.document_state.active_path = Some(path("top.snxsch"));
    app
}

/// `top` placing `child.snxsch` twice as sheets "A" (uuid 1) and "B" (uuid 2);
/// the child holds `R1` on a local net, optionally with a per-placement entry
/// giving B the designator `R101`.
pub(super) fn twice_placed_app(dir: &str, with_entries: bool) -> crate::app::Signex {
    let entries = if with_entries {
        vec![entry_for(&[1], "R1"), entry_for(&[2], "R101")]
    } else {
        Vec::new()
    };
    placed_app(dir, &["top.snxsch"], &["A", "B"], entries)
}

/// Scenario B: the child placed once as "A", its symbol renamed there.
pub(super) fn once_placed_app(dir: &str) -> crate::app::Signex {
    placed_app(dir, &["top.snxsch"], &["A"], vec![entry_for(&[1], "R101")])
}

#[test]
fn a_child_placed_twice_with_entries_exports_one_component_block_per_placement() {
    let app = twice_placed_app("/w/inst", true);
    let (ctx, issues) = build_export_scope(&app.document_state).expect("scope");
    assert_eq!(issues.shared_reference_refusal(&ctx), None);

    let out = NetlistExporter
        .export(&ctx, &NetlistOptions::default())
        .expect("distinct per-placement designators export");
    let text = String::from_utf8(out.bytes).unwrap();
    let (declared, referenced) = declared_and_referenced(&text);

    for designator in ["R1", "R101"] {
        assert_eq!(
            declared.iter().filter(|d| *d == designator).count(),
            1,
            "{designator} gets exactly one component block:\n{text}"
        );
    }
    assert!(text.contains("(\r\nA/CHILD_NET\r\nR1-1\r\n)"), "{text}");
    assert!(text.contains("(\r\nB/CHILD_NET\r\nR101-1\r\n)"), "{text}");
    for designator in &referenced {
        assert!(declared.contains(designator), "{designator}:\n{text}");
    }
}

#[test]
fn a_child_placed_twice_without_entries_is_still_refused() {
    let app = twice_placed_app("/w/noinst", false);
    let refusal = refusal_of(&app).expect("shared designators must still be refused");
    assert!(refusal.contains("R1"), "{refusal}");
}

#[test]
fn the_load_note_counts_only_changed_designators_once_per_project() {
    // R1's entry for placement A merely mirrors its reference: not counted.
    let mut app = twice_placed_app("/w/note", true);
    let scope = app.take_instance_note_scope().expect("first ask");
    assert_eq!(
        scope.note().as_deref(),
        Some("1 symbol placement takes its designator from a sheet-placement entry: R101")
    );
    assert!(app.take_instance_note_scope().is_none(), "once per project");

    let mut plain = twice_placed_app("/w/note-none", false);
    let scope = plain.take_instance_note_scope().expect("first ask");
    assert_eq!(scope.note(), None);
    assert!(plain.take_instance_note_scope().is_none());
}

fn entry_at_root(reference: &str) -> signex_types::schematic::SymbolInstance {
    signex_types::schematic::SymbolInstance {
        project: "p".to_string(),
        path: "/".to_string(),
        reference: reference.to_string(),
        unit: 1,
    }
}

#[test]
fn the_load_note_covers_a_declared_page_outside_the_hierarchy() {
    let mut app = app_workspace("/w/p2", &["top.snxsch", "p2.snxsch"]);
    let path = |name: &str| PathBuf::from("/w/p2").join(name);
    open_with(
        &mut app.document_state,
        &path("top.snxsch"),
        sheet_with_net("R_TOP", "TOP_NET", &[]),
    );
    let mut page2 = sheet_with_net("R2", "N2", &[]);
    page2.symbols[0].instances = vec![entry_at_root("R50")];
    open_with(&mut app.document_state, &path("p2.snxsch"), page2);
    app.document_state.active_path = Some(path("top.snxsch"));

    let scope = app.take_instance_note_scope().expect("first ask");
    assert_eq!(
        scope.note().as_deref(),
        Some("1 symbol placement takes its designator from a sheet-placement entry: R50")
    );
}

#[test]
fn the_load_note_covers_a_loose_document_once() {
    let mut app = app_workspace("/w/unused", &["x.snxsch"]);
    app.document_state.projects.clear();
    app.document_state.active_project = None;
    let loose = PathBuf::from("/w/loose/scratch.snxsch");
    let mut sheet = sheet_with_net("R1", "N", &[]);
    sheet.symbols[0].instances = vec![entry_at_root("R50")];
    open_with(&mut app.document_state, &loose, sheet);
    app.document_state.active_path = Some(loose);

    let scope = app.take_instance_note_scope().expect("first ask");
    assert_eq!(
        scope.note().as_deref(),
        Some("1 symbol placement takes its designator from a sheet-placement entry: R50")
    );
    assert!(app.take_instance_note_scope().is_none());
}

#[test]
fn closing_a_loose_document_forgets_that_its_note_was_shown() {
    let mut app = app_workspace("/w/unused", &["x.snxsch"]);
    app.document_state.projects.clear();
    app.document_state.active_project = None;
    let loose = PathBuf::from("/w/loose/reopened.snxsch");
    let mut sheet = sheet_with_net("R1", "N", &[]);
    sheet.symbols[0].instances = vec![entry_at_root("R50")];
    app.open_schematic_tab(loose.clone(), "reopened".into(), sheet.clone());
    assert!(app.take_instance_note_scope().is_some(), "first ask");

    let idx = app
        .document_state
        .tabs
        .iter()
        .position(|tab| tab.path == loose)
        .expect("the loose tab");
    let _ = app.close_tab_now(idx);
    app.open_schematic_tab(loose, "reopened".into(), sheet);

    assert!(
        app.take_instance_note_scope().is_some(),
        "reopening the sheet notes it again"
    );
}

#[test]
fn a_failed_note_walk_reaches_the_messages_panel() {
    let mut app = twice_placed_app("/w/note-failed", true);
    let reason = "instance-note-829-unique-join-failure";
    let before = crate::diagnostics::logged_count(reason);

    let _ = app.update(crate::app::Message::File(
        crate::app::FileMsg::InstanceNoteReady(Err(reason.to_string())),
    ));

    assert_eq!(
        crate::diagnostics::logged_count(reason),
        before + 1,
        "one Messages entry"
    );
}

#[test]
fn a_blank_root_entry_does_not_blank_the_designator_in_the_net_file() {
    // A default `[[instances]]` entry (empty path and reference) on a root
    // symbol must not make the Protel export refuse on a blank designator.
    let mut app = hierarchical_app();
    let top = PathBuf::from("/w/h").join("top.snxsch");
    let mut sheet = sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]);
    sheet.symbols[0].instances = vec![Default::default()];
    open_with(&mut app.document_state, &top, sheet);
    let (ctx, _) = build_export_scope(&app.document_state).expect("scope");
    let out = NetlistExporter
        .export(&ctx, &NetlistOptions::default())
        .expect("exports as on trunk");
    assert!(String::from_utf8(out.bytes).unwrap().contains("R_TOP"));
}

#[test]
fn a_project_without_entries_exports_exactly_the_pre_placement_file() {
    let app = hierarchical_app();
    let (ctx, _) = build_export_scope(&app.document_state).expect("scope");
    let out = NetlistExporter
        .export(&ctx, &NetlistOptions::default())
        .expect("exports");
    let text = String::from_utf8(out.bytes).unwrap();
    assert_eq!(text, GOLDEN_NO_ENTRIES);
}

/// Components in sheet-key order (`child.snxsch` before `top.snxsch`), then
/// the nets in id order — what a project of singly placed sheets produced
/// before placements existed.
const GOLDEN_NO_ENTRIES: &str = concat!(
    "[\r\nR_CHILD\r\n\r\n\r\n\r\n\r\n\r\n]\r\n",
    "[\r\nR_TOP\r\n\r\n\r\n\r\n\r\n\r\n]\r\n",
    "(\r\nTOP_NET\r\nR_TOP-1\r\n)\r\n",
    "(\r\nCHILD_NET\r\nR_CHILD-1\r\n)\r\n",
);

#[test]
fn a_child_placed_once_with_an_entry_exports_the_placement_designator() {
    let app = once_placed_app("/w/once");
    let (ctx, issues) = build_export_scope(&app.document_state).expect("scope");
    assert_eq!(issues.shared_reference_refusal(&ctx), None);
    let out = NetlistExporter
        .export(&ctx, &NetlistOptions::default())
        .expect("exports");
    let text = String::from_utf8(out.bytes).unwrap();
    let (declared, referenced) = declared_and_referenced(&text);
    assert!(declared.contains(&"R101".to_string()), "{text}");
    assert!(!declared.contains(&"R1".to_string()), "{text}");
    assert!(referenced.contains(&"R101".to_string()), "{text}");
}

/// `top` places `mid.snxsch` twice (A=1, B=2); `mid` places `leaf.snxsch` once
/// as X (uuid 10); the leaf's `R1` is R1 at /A/X and R201 at /B/X.
#[test]
fn a_grandchild_gets_its_own_designator_under_each_parent_placement() {
    use signex_types::schematic::LabelType;
    let dir = "/w/nested";
    let mut app = app_workspace(dir, &["top.snxsch"]);
    let path = |name: &str| PathBuf::from(dir).join(name);
    let mut top = sheet_with_net("R_TOP", "TOP_NET", &["mid.snxsch", "mid.snxsch"]);
    for (n, (cs, name)) in top.child_sheets.iter_mut().zip(["A", "B"]).enumerate() {
        cs.name = name.to_string();
        cs.uuid = Uuid::from_u128(n as u128 + 1);
    }
    open_with(&mut app.document_state, &path("top.snxsch"), top);
    let mut mid = sheet_with_net("R_MID", "MID_NET", &["leaf.snxsch"]);
    mid.child_sheets[0].name = "X".to_string();
    mid.child_sheets[0].uuid = Uuid::from_u128(10);
    mid.symbols[0].instances = vec![entry_for(&[1], "R_MID"), entry_for(&[2], "R_MID2")];
    mid.labels[0].label_type = LabelType::Net;
    open_with(&mut app.document_state, &path("mid.snxsch"), mid);
    let mut leaf = sheet_with_net("R1", "LEAF_NET", &[]);
    leaf.labels[0].label_type = LabelType::Net;
    leaf.symbols[0].instances = vec![entry_for(&[1, 10], "R1"), entry_for(&[2, 10], "R201")];
    open_with(&mut app.document_state, &path("leaf.snxsch"), leaf);
    app.document_state.active_path = Some(path("top.snxsch"));

    let (ctx, issues) = build_export_scope(&app.document_state).expect("scope");
    assert_eq!(issues.shared_reference_refusal(&ctx), None);
    let text = String::from_utf8(
        NetlistExporter
            .export(&ctx, &NetlistOptions::default())
            .expect("exports")
            .bytes,
    )
    .unwrap();
    let (declared, _) = declared_and_referenced(&text);
    for designator in ["R1", "R201", "R_MID", "R_MID2"] {
        assert_eq!(
            declared.iter().filter(|d| *d == designator).count(),
            1,
            "{designator}:\n{text}"
        );
    }
    assert!(text.contains("A/X/LEAF_NET\r\nR1-1"), "{text}");
    assert!(text.contains("B/X/LEAF_NET\r\nR201-1"), "{text}");
}

#[test]
fn printed_pages_are_one_copy_per_placement_with_resolved_designators() {
    use crate::app::handlers::menu::export::build_page_scope;
    let app = placed_app(
        "/w/pdf",
        &["top.snxsch", "child.snxsch"],
        &["A", "B"],
        vec![entry_for(&[1], "R1"), entry_for(&[2], "R101")],
    );
    let (ctx, _) = build_page_scope(&app.document_state).expect("scope");
    let shape: Vec<(String, String, Vec<String>, usize, usize)> = ctx
        .sheets
        .iter()
        .map(|p| {
            (
                p.sheet_name.clone(),
                p.placement.to_string(),
                p.schematic
                    .symbols
                    .iter()
                    .map(|s| s.reference.clone())
                    .collect(),
                p.sheet_number,
                p.sheet_count,
            )
        })
        .collect();
    let id = |n: u128| Uuid::from_u128(n);
    assert_eq!(shape.len(), 3, "{shape:?}");
    assert_eq!(shape[0].1, "/", "{shape:?}");
    assert_eq!(shape[1].0, "child.snxsch (A)", "{shape:?}");
    assert_eq!(shape[1].1, format!("/{}", id(1)));
    assert_eq!(shape[1].2, ["R1"]);
    assert_eq!(shape[2].0, "child.snxsch (B)", "{shape:?}");
    assert_eq!(shape[2].1, format!("/{}", id(2)));
    assert_eq!(shape[2].2, ["R101"]);
    assert_eq!((shape[2].3, shape[2].4), (3, 3), "renumbered");
}

#[test]
fn a_printed_child_placed_once_prints_its_placement_designator() {
    use crate::app::handlers::menu::export::build_page_scope;
    let app = placed_app(
        "/w/pdf1",
        &["top.snxsch", "child.snxsch"],
        &["A"],
        vec![entry_for(&[1], "R101")],
    );
    let (ctx, _) = build_page_scope(&app.document_state).expect("scope");
    assert_eq!(ctx.sheets.len(), 2);
    assert_eq!(ctx.sheets[1].schematic.symbols[0].reference, "R101");
    assert_eq!(ctx.sheets[1].sheet_number, 2, "numbering untouched");
}

/// "Current page" while viewing a declared child placed twice that was opened
/// without descending (no recorded placement): the first page copy is current.
#[test]
fn current_page_on_a_declared_child_placed_twice_is_its_first_copy() {
    use crate::app::handlers::menu::export::build_page_scope;
    let mut app = placed_app(
        "/w/cur",
        &["top.snxsch", "child.snxsch"],
        &["A", "B"],
        vec![entry_for(&[1], "R7"), entry_for(&[2], "R101")],
    );
    app.document_state.active_path = Some(PathBuf::from("/w/cur").join("child.snxsch"));
    let (ctx, _) = build_page_scope(&app.document_state).expect("scope");
    let current = &ctx.sheets[ctx.current_sheet_index()];
    assert_eq!(current.sheet_name, "child.snxsch (A)");
    assert_eq!(current.schematic.symbols[0].reference, "R7");
}

/// Viewing a child that is not a declared page and was opened without
/// descending: the isolated page is the live sheet at the smallest placement,
/// with that placement's designators.
#[test]
fn current_page_on_an_undeclared_child_placed_twice_resolves_a_placement() {
    use crate::app::handlers::menu::export::{build_page_scope, isolate_undeclared_current};
    let mut app = placed_app(
        "/w/cur2",
        &["top.snxsch"],
        &["A", "B"],
        vec![entry_for(&[1], "R7"), entry_for(&[2], "R101")],
    );
    let child = PathBuf::from("/w/cur2").join("child.snxsch");
    app.document_state.active_path = Some(child.clone());
    let (mut ctx, issues) = build_page_scope(&app.document_state).expect("scope");
    assert!(isolate_undeclared_current(
        &mut ctx,
        &issues,
        &app.document_state,
        &child,
        app.document_state.tab_placements.get(&child),
        &signex_output::PageRange::Current
    ));
    assert_eq!(ctx.sheets.len(), 1);
    assert_eq!(ctx.sheets[0].schematic.symbols[0].reference, "R7");
    assert_eq!(
        ctx.sheets[0].placement.to_string(),
        format!("/{}", Uuid::from_u128(1))
    );
}

/// Any schematic open funnels through `open_schematic_tab`, which marks the
/// note due; the shared `finish_update` then starts it (once per project).
#[test]
fn opening_a_schematic_tab_marks_the_designator_note_due() {
    let mut app = twice_placed_app("/w/due", true);
    assert!(!app.document_state.instance_note_due);
    let sheet = app
        .document_state
        .engines
        .values()
        .next()
        .expect("an open engine")
        .document()
        .clone();
    app.open_schematic_tab(PathBuf::from("/w/due/top.snxsch"), "top".into(), sheet);
    assert!(app.document_state.instance_note_due);
    let _ = app.finish_update();
    assert!(!app.document_state.instance_note_due, "drained once");
}

#[test]
fn a_singly_placed_printed_child_keeps_its_page_name() {
    use crate::app::handlers::menu::export::build_page_scope;
    let app = placed_app(
        "/w/name1",
        &["top.snxsch", "child.snxsch"],
        &["A"],
        vec![entry_for(&[1], "R101")],
    );
    let (ctx, _) = build_page_scope(&app.document_state).expect("scope");
    assert_eq!(ctx.sheets[1].sheet_name, "child.snxsch");
    assert_eq!(ctx.sheets[1].schematic.symbols[0].reference, "R101");
}

#[test]
fn two_copies_of_a_child_are_two_independent_picker_rows() {
    use crate::app::handlers::menu::export::build_page_scope;
    let mut app = placed_app(
        "/w/pick",
        &["top.snxsch", "child.snxsch"],
        &["A", "B"],
        vec![entry_for(&[1], "R1"), entry_for(&[2], "R101")],
    );
    let _ = build_page_scope(&app.document_state).expect("scope");
    let _ = app.handle_print_preview_requested();
    let preview = app.document_state.preview.as_ref().expect("preview");
    let rows: Vec<_> = preview.sheet_files.iter().map(|(k, _)| k.clone()).collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].0, rows[2].0, "one file");
    assert_ne!(rows[1].1, rows[2].1, "two placements");
    let second = rows[2].clone();
    let _ = app.handle_print_preview_toggle_file(second.clone());
    let preview = app.document_state.preview.as_ref().expect("preview");
    assert!(!preview.selected_files.contains(&second));
    assert!(
        preview.selected_files.contains(&rows[1]),
        "the other copy stays ticked"
    );
}
