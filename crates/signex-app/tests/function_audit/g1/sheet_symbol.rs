//! Mockup row 46: a sheet symbol's Properties.

use super::support::{Bench, bench, empty_sheet};
use signex_app::panels::{PanelMsg, PlacedField};
use signex_types::schematic::{
    ChildSheet, FillType, Point, SelectedItem, SelectedKind, StrokeColor,
};
use uuid::Uuid;

fn child(name: &str, file: &str, x: f64) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::now_v7(),
        name: name.into(),
        filename: file.into(),
        position: Point::new(x, 20.0),
        size: (40.0, 30.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn sheet_bench() -> (Bench, Uuid, SelectedItem) {
    let mut sheet = empty_sheet();
    sheet
        .child_sheets
        .push(child("Power", "power.snxsch", 20.0));
    sheet
        .child_sheets
        .push(child("Logic", "logic.snxsch", 100.0));
    let id = sheet.child_sheets[0].uuid;
    (
        bench(sheet),
        id,
        SelectedItem::new(id, SelectedKind::ChildSheet),
    )
}

fn first(b: &Bench) -> &ChildSheet {
    &b.sheet().child_sheets[0]
}

fn commit_name(b: &mut Bench, id: Uuid, text: &str) {
    b.panel(PanelMsg::ChildSheetNameTyping(id, text.to_string()));
    b.panel(PanelMsg::CommitChildSheetName(id));
}

fn commit_file(b: &mut Bench, id: Uuid, text: &str) {
    b.panel(PanelMsg::ChildSheetFileTyping(id, text.to_string()));
    b.panel(PanelMsg::CommitChildSheetFile(id));
}

fn commit_line_width(b: &mut Bench, id: Uuid, text: &str) {
    b.panel(PanelMsg::ChildSheetStrokeWidthTyping(id, text.to_string()));
    b.panel(PanelMsg::CommitChildSheetStrokeWidth(id));
}

#[test]
fn m46_the_name_applies_on_enter_and_undoes() {
    let (mut b, id, _) = sheet_bench();
    b.panel(PanelMsg::ChildSheetNameTyping(id, "Supply".into()));
    assert_eq!(first(&b).name, "Power", "typing alone changes nothing");
    b.panel(PanelMsg::CommitChildSheetName(id));
    assert_eq!(first(&b).name, "Supply");
    assert_eq!(first(&b).filename, "power.snxsch", "the file stays");
    assert!(b.dirty());
    b.undo();
    assert_eq!(first(&b).name, "Power");
    assert!(!b.dirty());
}

#[test]
fn m46_a_blank_or_taken_name_is_refused_with_a_notice() {
    let (mut b, id, _) = sheet_bench();
    for bad in ["   ", "LOGIC"] {
        b.app.document_state.error_notice = None;
        commit_name(&mut b, id, bad);
        assert!(b.app.document_state.error_notice.is_some(), "{bad:?}");
        assert_eq!(first(&b).name, "Power", "{bad:?}");
    }
    assert!(!b.dirty());
}

#[test]
fn m46_the_file_repoints_the_symbol_and_writes_nothing_to_disk() {
    let (mut b, id, _) = sheet_bench();
    commit_file(&mut b, id, "sub/supply.snxsch");
    assert_eq!(first(&b).filename, "sub/supply.snxsch");
    assert_eq!(first(&b).name, "Power", "the name stays");
    let folder = b.path.parent().expect("a folder");
    assert!(!folder.join("sub").exists(), "no file or folder is created");
    b.undo();
    assert_eq!(first(&b).filename, "power.snxsch");
}

#[test]
fn m46_a_file_that_is_not_a_sheet_or_leaves_the_folder_is_refused() {
    let (mut b, id, _) = sheet_bench();
    for bad in ["", "power.txt", "../evil.snxsch"] {
        b.app.document_state.error_notice = None;
        commit_file(&mut b, id, bad);
        assert!(b.app.document_state.error_notice.is_some(), "{bad:?}");
        assert_eq!(first(&b).filename, "power.snxsch", "{bad:?}");
    }
    assert!(!b.dirty());
}

#[test]
fn m46_width_and_height_resize_the_symbol_each_in_one_undo_step() {
    let (mut b, _, item) = sheet_bench();
    b.enter(item, PlacedField::Width, "50");
    b.enter(item, PlacedField::Height, "35");
    assert_eq!(first(&b).size, (50.0, 35.0));
    b.undo();
    assert_eq!(first(&b).size, (50.0, 30.0));
    b.undo();
    assert_eq!(first(&b).size, (40.0, 30.0));
    assert!(!b.dirty());
}

#[test]
fn m46_a_zero_width_is_refused_and_the_text_stays() {
    let (mut b, _, item) = sheet_bench();
    b.enter(item, PlacedField::Width, "0");
    assert_eq!(first(&b).size, (40.0, 30.0));
    assert!(b.has_draft());
    assert!(!b.dirty());
}

#[test]
fn m46_a_typed_position_moves_the_symbol() {
    let (mut b, _, item) = sheet_bench();
    b.enter(item, PlacedField::X, "25");
    assert_eq!(first(&b).position.x, 25.0);
    b.undo();
    assert_eq!(first(&b).position.x, 20.0);
}

#[test]
fn m46_line_width_applies_on_enter_and_empty_means_the_default() {
    let (mut b, id, _) = sheet_bench();
    commit_line_width(&mut b, id, "0.5");
    assert_eq!(first(&b).stroke_width, 0.5);
    commit_line_width(&mut b, id, "");
    assert_eq!(first(&b).stroke_width, 0.0);
    b.undo();
    assert_eq!(first(&b).stroke_width, 0.5);
}

#[test]
fn m46_a_negative_line_width_is_refused_and_the_text_stays_to_fix() {
    let (mut b, id, _) = sheet_bench();
    commit_line_width(&mut b, id, "-1");
    assert_eq!(first(&b).stroke_width, 0.0);
    assert!(!b.dirty());
    assert_eq!(
        b.app
            .document_state
            .panel_ctx
            .child_sheet_stroke_width_buf
            .as_deref(),
        Some("-1"),
        "a refused edit keeps what was typed"
    );
}

#[test]
fn m46_border_and_fill_colours_apply_and_reset_to_default_clears_the_style() {
    let (mut b, id, _) = sheet_bench();
    let red = StrokeColor {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    b.panel(PanelMsg::ToggleChildSheetBorderPicker(id));
    b.panel(PanelMsg::EditChildSheetBorderColor(
        id,
        iced::Color::from_rgb8(255, 0, 0),
    ));
    assert_eq!(first(&b).stroke_color, Some(red));
    assert!(
        !b.app
            .document_state
            .panel_ctx
            .child_sheet_border_picker_open
    );
    b.panel(PanelMsg::EditChildSheetFillColor(
        id,
        iced::Color::from_rgb8(255, 0, 0),
    ));
    assert_eq!(first(&b).fill_color, Some(red));
    commit_line_width(&mut b, id, "0.5");

    b.panel(PanelMsg::ResetChildSheetStyle(id));
    assert_eq!(first(&b).stroke_color, None);
    assert_eq!(first(&b).fill_color, None);
    assert_eq!(first(&b).stroke_width, 0.0);
    b.undo();
    assert_eq!(first(&b).stroke_color, Some(red), "reset is one undo step");
    assert_eq!(first(&b).stroke_width, 0.5);
}

#[test]
fn m46_cancelling_a_colour_picker_leaves_the_symbol() {
    let (mut b, id, _) = sheet_bench();
    b.panel(PanelMsg::ToggleChildSheetFillPicker(id));
    b.panel(PanelMsg::CancelChildSheetColorPicker);
    assert!(!b.app.document_state.panel_ctx.child_sheet_fill_picker_open);
    assert_eq!(first(&b).fill_color, None);
    assert!(!b.dirty());
}
