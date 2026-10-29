//! #1258 — Properties edits the fields a multi-selection shares: a differing
//! value reads `<mixed>`, one change reaches every object of its kind, and
//! the whole change is one undo step.

use crate::field_edit_support::{
    app_with, doc, empty_sheet, label, logged, part, select_everything, wire,
};
use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{MultiEditMsg, MultiField, PanelMsg};
use signex_engine::{MultiEdit, Shared};
use signex_types::schematic::HAlign;

fn send(app: &mut Signex, msg: MultiEditMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::MultiEdit(msg))));
}

fn type_and_submit(app: &mut Signex, field: MultiField, text: &str) {
    send(app, MultiEditMsg::DraftChanged(field.clone(), text.into()));
    send(app, MultiEditMsg::DraftSubmitted(field));
}

fn all_kinds() -> Signex {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![
        part("R1", "10k", (20.0, 20.0)),
        part("R2", "22k", (60.0, 20.0)),
        part("R3", "10k", (100.0, 20.0)),
    ];
    sheet.labels = vec![label("A", (20.0, 80.0)), label("B", (60.0, 80.0))];
    sheet.wires = vec![
        wire((20.0, 120.0), (40.0, 120.0), 0.0),
        wire((60.0, 120.0), (80.0, 120.0), 0.3),
    ];
    let mut app = app_with(sheet);
    select_everything(&mut app);
    app
}

fn values(app: &Signex) -> Vec<String> {
    doc(app).symbols.iter().map(|s| s.value.clone()).collect()
}

#[test]
fn the_panel_groups_the_selection_by_kind_and_marks_differences_mixed() {
    let app = all_kinds();
    let groups = &app.document_state.panel_ctx.multi_groups;
    let symbols = groups.symbols.as_ref().expect("symbols");
    assert_eq!(symbols.count, 3, "the designator fields are not parts");
    assert_eq!(symbols.value, Shared::Mixed);
    assert_eq!(symbols.footprint, Shared::Same("R_0603".into()));
    assert_eq!(groups.labels.as_ref().unwrap().count, 2);
    assert_eq!(groups.wires.as_ref().unwrap().width_mm, Shared::Mixed);
}

#[test]
fn one_typed_value_reaches_every_part_and_one_undo_takes_it_back() {
    let mut app = all_kinds();
    type_and_submit(&mut app, MultiField::SymbolValue, "4k7");
    assert_eq!(values(&app), ["4k7", "4k7", "4k7"]);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(values(&app), ["10k", "22k", "10k"], "one undo step");
    assert!(
        app.document_state.panel_ctx.multi_draft.is_none(),
        "the draft ends with the edit"
    );
}

#[test]
fn typing_alone_changes_nothing_until_enter() {
    let mut app = all_kinds();
    send(
        &mut app,
        MultiEditMsg::DraftChanged(MultiField::SymbolValue, "4k7".into()),
    );
    assert_eq!(values(&app), ["10k", "22k", "10k"]);
}

#[test]
fn a_label_edit_leaves_parts_and_wires_alone() {
    let mut app = all_kinds();
    type_and_submit(&mut app, MultiField::LabelText, "NET1");
    send(
        &mut app,
        MultiEditMsg::Picked(MultiEdit::LabelJustify(HAlign::Right)),
    );
    assert!(
        doc(&app)
            .labels
            .iter()
            .all(|l| l.text == "NET1" && l.justify == HAlign::Right)
    );
    assert_eq!(values(&app), ["10k", "22k", "10k"]);
    assert_eq!(doc(&app).wires[1].stroke_width, 0.3);
}

#[test]
fn wire_width_and_part_flags_apply_to_all() {
    let mut app = all_kinds();
    type_and_submit(&mut app, MultiField::WireWidth, "0.5");
    assert!(
        doc(&app)
            .wires
            .iter()
            .all(|w| (w.stroke_width - 0.5).abs() < 1e-9)
    );
    send(&mut app, MultiEditMsg::Picked(MultiEdit::SymbolDnp(true)));
    assert!(doc(&app).symbols.iter().all(|s| s.dnp));
    send(
        &mut app,
        MultiEditMsg::Picked(MultiEdit::SymbolInBom(false)),
    );
    assert!(doc(&app).symbols.iter().all(|s| !s.in_bom));
}

#[test]
fn text_that_is_not_a_number_is_refused_in_the_log_and_kept_as_the_draft() {
    let mut app = all_kinds();
    type_and_submit(&mut app, MultiField::WireWidth, "wide-9183");
    assert!(logged(&["Wire width", "wide-9183", "not a number"]));
    assert_eq!(doc(&app).wires[1].stroke_width, 0.3);
    assert_eq!(
        app.document_state.panel_ctx.multi_draft,
        Some((MultiField::WireWidth, "wide-9183".into())),
        "the text stays for the user to fix"
    );
}

#[test]
fn a_locked_part_is_skipped_and_the_log_says_so() {
    let mut app = all_kinds();
    let locked = doc(&app).symbols[1].uuid;
    app.document_state
        .active_engine_mut()
        .unwrap()
        .execute(signex_engine::Command::SetSymbolLocked {
            symbol_id: locked,
            locked: true,
        })
        .unwrap();
    select_everything(&mut app);
    type_and_submit(&mut app, MultiField::SymbolFootprint, "R_0402-8341");
    let footprints: Vec<_> = doc(&app)
        .symbols
        .iter()
        .map(|s| s.footprint.clone())
        .collect();
    assert_eq!(footprints, ["R_0402-8341", "R_0603", "R_0402-8341"]);
    assert!(logged(&["Skipped 1 locked symbol(s)"]));
}
