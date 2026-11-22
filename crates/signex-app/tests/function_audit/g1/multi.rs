//! Mockup row 51: Properties of a multi-selection.

use super::support::{Bench, bench, empty_sheet, label, part, wire};
use signex_app::panels::{MultiEditMsg, MultiField, MultiKind, PanelMsg, SelectionFormMsg};
use signex_engine::{MultiEdit, Shared};
use signex_types::schematic::{HAlign, NoConnect, Point, WireWidthPreset};
use uuid::Uuid;

fn mixed() -> Bench {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![
        part("R1", "10k", (20.0, 20.0)),
        part("R2", "22k", (60.0, 20.0)),
    ];
    sheet.labels = vec![label("A", (20.0, 80.0)), label("B", (60.0, 80.0))];
    sheet.wires = vec![
        wire((20.0, 120.0), (40.0, 120.0), 0.0),
        wire((60.0, 120.0), (80.0, 120.0), 0.3),
    ];
    let mut b = bench(sheet);
    b.select_everything();
    b
}

fn send(b: &mut Bench, msg: MultiEditMsg) {
    b.panel(PanelMsg::MultiEdit(msg));
}

fn type_and_submit(b: &mut Bench, field: MultiField, text: &str) {
    send(b, MultiEditMsg::DraftChanged(field.clone(), text.into()));
    send(b, MultiEditMsg::DraftSubmitted(field));
}

fn values(b: &Bench) -> Vec<String> {
    b.sheet().symbols.iter().map(|s| s.value.clone()).collect()
}

fn widths(b: &Bench) -> Vec<f64> {
    b.sheet().wires.iter().map(|w| w.stroke_width).collect()
}

#[test]
fn m51_the_form_groups_by_kind_and_marks_differing_values_mixed() {
    let b = mixed();
    let groups = &b.app.document_state.panel_ctx.multi_groups;
    let symbols = groups.symbols.as_ref().expect("parts");
    assert_eq!(symbols.count, 2);
    assert_eq!(symbols.value, Shared::Mixed);
    assert_eq!(symbols.footprint, Shared::Same("R_0603".to_string()));
    assert_eq!(groups.labels.as_ref().expect("labels").count, 2);
    assert_eq!(
        groups.wires.as_ref().expect("wires").width_mm,
        Shared::Mixed
    );
}

#[test]
fn m51_a_typed_value_reaches_every_part_on_enter_in_one_undo_step() {
    let mut b = mixed();
    send(
        &mut b,
        MultiEditMsg::DraftChanged(MultiField::SymbolValue, "4k7".into()),
    );
    assert_eq!(values(&b), ["10k", "22k"], "typing alone changes nothing");
    assert!(!b.dirty());
    send(
        &mut b,
        MultiEditMsg::DraftSubmitted(MultiField::SymbolValue),
    );
    assert_eq!(values(&b), ["4k7", "4k7"]);
    assert!(b.dirty());
    b.undo();
    assert_eq!(values(&b), ["10k", "22k"]);
    assert!(!b.dirty());
}

#[test]
fn m51_a_wire_width_preset_sets_every_wire_in_one_undo_step() {
    let mut b = mixed();
    let large = WireWidthPreset::Large.stroke_mm();
    send(&mut b, MultiEditMsg::Picked(MultiEdit::WireWidthMm(large)));
    assert_eq!(widths(&b), [large, large]);
    b.undo();
    assert_eq!(widths(&b), [0.0, 0.3]);
}

#[test]
fn m51_a_negative_wire_width_is_refused_and_the_text_stays() {
    let mut b = mixed();
    type_and_submit(&mut b, MultiField::WireWidth, "-1");
    assert_eq!(widths(&b), [0.0, 0.3]);
    assert!(!b.dirty());
    assert!(b.app.document_state.panel_ctx.multi_draft.is_some());
}

#[test]
fn m51_a_label_font_size_of_zero_is_refused() {
    let mut b = mixed();
    let before: Vec<f64> = b.sheet().labels.iter().map(|l| l.font_size).collect();
    type_and_submit(&mut b, MultiField::LabelFontPt, "0");
    let after: Vec<f64> = b.sheet().labels.iter().map(|l| l.font_size).collect();
    assert_eq!(after, before);
    assert!(!b.dirty());
}

#[test]
fn m51_a_label_edit_never_renames_the_labels() {
    let mut b = mixed();
    send(
        &mut b,
        MultiEditMsg::Picked(MultiEdit::LabelJustify(HAlign::Right)),
    );
    send(
        &mut b,
        MultiEditMsg::Picked(MultiEdit::LabelRotationDegrees(90.0)),
    );
    let labels = &b.sheet().labels;
    assert!(
        labels
            .iter()
            .all(|l| l.justify == HAlign::Right && l.rotation == 90.0)
    );
    let names: Vec<&str> = labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(names, ["A", "B"]);
}

#[test]
fn m51_the_part_flags_reach_every_part() {
    let mut b = mixed();
    send(&mut b, MultiEditMsg::Picked(MultiEdit::SymbolDnp(true)));
    send(&mut b, MultiEditMsg::Picked(MultiEdit::SymbolInBom(false)));
    send(
        &mut b,
        MultiEditMsg::Picked(MultiEdit::SymbolOnBoard(false)),
    );
    assert!(
        b.sheet()
            .symbols
            .iter()
            .all(|s| s.dnp && !s.in_bom && !s.on_board)
    );
}

#[test]
fn m51_a_kind_chip_narrows_the_sections_and_not_the_selection() {
    let mut b = mixed();
    let before = b.app.interaction_state.active_canvas().selected.clone();
    b.panel(PanelMsg::SelectionForm(SelectionFormMsg::KindChipPicked(
        Some(MultiKind::Wires),
    )));
    let ctx = &b.app.document_state.panel_ctx;
    assert!(
        ctx.selection_form
            .shows(MultiKind::Wires, &ctx.multi_groups)
    );
    assert!(
        !ctx.selection_form
            .shows(MultiKind::Parts, &ctx.multi_groups)
    );
    assert_eq!(b.app.interaction_state.active_canvas().selected, before);
    assert!(!b.dirty());
}

#[test]
fn m51_a_selection_with_nothing_shared_has_no_fields_and_edits_nothing() {
    let mut sheet = empty_sheet();
    for x in [20.0, 40.0] {
        sheet.no_connects.push(NoConnect {
            uuid: Uuid::now_v7(),
            position: Point::new(x, 20.0),
        });
    }
    let mut b = bench(sheet);
    b.select_everything();
    assert_eq!(b.app.interaction_state.active_canvas().selected.len(), 2);
    assert!(b.app.document_state.panel_ctx.multi_groups.is_empty());
    send(&mut b, MultiEditMsg::Picked(MultiEdit::SymbolDnp(true)));
    assert!(!b.dirty());
}
