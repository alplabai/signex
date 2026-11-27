//! #1699 (mockup rows 46, 48, 49, 51): the schematic Properties forms. A sheet
//! symbol's size is one undo step and an emptied Line Width sets the
//! default back; the multi-selection kind chips only choose the sections
//! shown, and a wire width preset reaches every wire in one undo step.

use crate::field_edit_support::{app_with, doc, empty_sheet, label, part, select_everything, wire};
use signex_app::app::{EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{
    MultiEditMsg, MultiKind, PanelMsg, PlacedEditMsg, PlacedField, SelectionFormMsg,
};
use signex_engine::MultiEdit;
use signex_types::schematic::{
    ChildSheet, FillType, Point, SelectedItem, SelectedKind, WireWidthPreset,
};

fn send(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn sheet_symbol_app() -> (Signex, uuid::Uuid) {
    let mut sheet = empty_sheet();
    let id = uuid::Uuid::now_v7();
    sheet.child_sheets.push(ChildSheet {
        uuid: id,
        name: "Power".into(),
        filename: "power.snxsch".into(),
        position: Point::new(20.0, 20.0),
        size: (40.0, 30.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    (app_with(sheet), id)
}

#[test]
fn a_sheet_symbol_width_edit_is_one_undo_step() {
    let (mut app, id) = sheet_symbol_app();
    let item = SelectedItem::new(id, SelectedKind::ChildSheet);
    send(
        &mut app,
        PanelMsg::PlacedEdit(PlacedEditMsg::Typed {
            item,
            field: PlacedField::Width,
            text: "50".into(),
        }),
    );
    send(
        &mut app,
        PanelMsg::PlacedEdit(PlacedEditMsg::Submitted {
            item,
            field: PlacedField::Width,
        }),
    );
    assert_eq!(doc(&app).child_sheets[0].size, (50.0, 30.0));
    undo(&mut app);
    assert_eq!(
        doc(&app).child_sheets[0].size,
        (40.0, 30.0),
        "one undo step"
    );
}

#[test]
fn an_emptied_line_width_sets_the_default_back() {
    let (mut app, id) = sheet_symbol_app();
    send(
        &mut app,
        PanelMsg::ChildSheetStrokeWidthTyping(id, "0.5".into()),
    );
    send(&mut app, PanelMsg::CommitChildSheetStrokeWidth(id));
    assert_eq!(doc(&app).child_sheets[0].stroke_width, 0.5);
    send(
        &mut app,
        PanelMsg::ChildSheetStrokeWidthTyping(id, String::new()),
    );
    send(&mut app, PanelMsg::CommitChildSheetStrokeWidth(id));
    assert_eq!(
        doc(&app).child_sheets[0].stroke_width,
        0.0,
        "empty is the default"
    );
}

fn mixed_app() -> Signex {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![part("R1", "10k", (20.0, 20.0))];
    sheet.labels = vec![label("A", (20.0, 80.0))];
    sheet.wires = vec![
        wire((20.0, 120.0), (40.0, 120.0), 0.0),
        wire((60.0, 120.0), (80.0, 120.0), 0.3),
    ];
    let mut app = app_with(sheet);
    select_everything(&mut app);
    app
}

#[test]
fn a_kind_chip_filters_the_sections_and_leaves_the_selection_alone() {
    let mut app = mixed_app();
    let before = app.interaction_state.active_canvas().selected.clone();
    send(
        &mut app,
        PanelMsg::SelectionForm(SelectionFormMsg::KindChipPicked(Some(MultiKind::Wires))),
    );
    assert_eq!(
        app.document_view.panel_ctx.selection_form.kind_filter,
        Some(MultiKind::Wires)
    );
    assert_eq!(app.interaction_state.active_canvas().selected, before);
    let groups = &app.document_view.panel_ctx.multi_groups;
    let form = &app.document_view.panel_ctx.selection_form;
    assert!(form.shows(MultiKind::Wires, groups));
    assert!(!form.shows(MultiKind::Parts, groups));
}

#[test]
fn a_mixed_wire_width_preset_sets_every_wire_in_one_undo_step() {
    let mut app = mixed_app();
    let widths =
        |app: &Signex| -> Vec<f64> { doc(app).wires.iter().map(|w| w.stroke_width).collect() };
    let large = WireWidthPreset::Large.stroke_mm();
    send(
        &mut app,
        PanelMsg::MultiEdit(MultiEditMsg::Picked(MultiEdit::WireWidthMm(large))),
    );
    assert_eq!(widths(&app), [large, large]);
    undo(&mut app);
    assert_eq!(widths(&app), [0.0, 0.3], "one undo step");
}
