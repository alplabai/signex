//! Mockup row 47: a text note's Properties.

use super::support::{Bench, bench, empty_sheet};
use iced::widget::text_editor::{Action, Edit, Motion};
use signex_app::panels::{NoteLayout, PanelMsg, PlacedEditMsg, PlacedField};
use signex_types::schematic::{HAlign, Point, SelectedItem, SelectedKind, TextNote, VAlign};
use uuid::Uuid;

fn note_bench() -> (Bench, SelectedItem) {
    let mut sheet = empty_sheet();
    let id = Uuid::now_v7();
    sheet.text_notes.push(TextNote {
        uuid: id,
        text: "Hello".into(),
        position: Point::new(40.0, 40.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
    });
    let mut b = bench(sheet);
    b.select_everything();
    let item = SelectedItem::new(id, SelectedKind::TextNote);
    assert_eq!(
        b.app.interaction_state.active_canvas().selected,
        vec![item],
        "the note alone is selected"
    );
    (b, item)
}

fn note(b: &Bench) -> &TextNote {
    &b.sheet().text_notes[0]
}

fn act(b: &mut Bench, item: SelectedItem, action: Action) {
    b.panel(PanelMsg::PlacedEdit(PlacedEditMsg::NoteActed {
        item,
        action,
    }));
}

fn type_at_end(b: &mut Bench, item: SelectedItem, text: &str) {
    act(b, item, Action::Move(Motion::DocumentEnd));
    for c in text.chars() {
        let edit = if c == '\n' {
            Edit::Enter
        } else {
            Edit::Insert(c)
        };
        act(b, item, Action::Edit(edit));
    }
}

fn submit(b: &mut Bench, item: SelectedItem) {
    b.panel(PanelMsg::PlacedEdit(PlacedEditMsg::NoteSubmitted { item }));
}

fn layout(b: &mut Bench, item: SelectedItem, layout: NoteLayout) {
    b.panel(PanelMsg::PlacedEdit(PlacedEditMsg::NoteLayoutPicked {
        item,
        layout,
    }));
}

#[test]
fn m47_multi_line_text_applies_on_submit_as_one_undo_step() {
    let (mut b, item) = note_bench();
    type_at_end(&mut b, item, "\nworld");
    assert_eq!(note(&b).text, "Hello", "typing alone leaves the sheet");
    assert!(!b.dirty());
    submit(&mut b, item);
    assert_eq!(note(&b).text, "Hello\nworld");
    assert!(b.dirty());
    b.undo();
    assert_eq!(note(&b).text, "Hello");
    assert!(!b.dirty());
}

#[test]
fn m47_submitting_an_untouched_box_is_no_edit() {
    let (mut b, item) = note_bench();
    submit(&mut b, item);
    assert_eq!(note(&b).text, "Hello");
    assert!(!b.dirty());
}

#[test]
fn m47_justify_and_rotation_each_apply_in_one_undo_step() {
    let (mut b, item) = note_bench();
    layout(&mut b, item, NoteLayout::JustifyH(HAlign::Right));
    layout(&mut b, item, NoteLayout::JustifyV(VAlign::Top));
    layout(&mut b, item, NoteLayout::Rotation(90.0));
    assert_eq!(note(&b).justify_h, HAlign::Right);
    assert_eq!(note(&b).justify_v, VAlign::Top);
    assert_eq!(note(&b).rotation, 90.0);
    b.undo();
    assert_eq!(note(&b).rotation, 0.0);
    b.undo();
    assert_eq!(note(&b).justify_v, VAlign::Bottom);
    b.undo();
    assert_eq!(note(&b).justify_h, HAlign::Left);
    assert!(!b.dirty());
}

#[test]
fn m47_the_text_size_applies_on_enter() {
    let (mut b, item) = note_bench();
    b.enter(item, PlacedField::TextSize, "2.54");
    assert_eq!(note(&b).font_size, 2.54);
    b.undo();
    assert_eq!(note(&b).font_size, 1.27);
}

#[test]
fn m47_a_zero_text_size_is_refused_and_the_text_stays() {
    let (mut b, item) = note_bench();
    b.enter(item, PlacedField::TextSize, "0");
    assert_eq!(note(&b).font_size, 1.27);
    assert!(b.has_draft());
    assert!(!b.dirty());
}

#[test]
fn m47_a_typed_position_moves_the_note() {
    let (mut b, item) = note_bench();
    b.enter(item, PlacedField::X, "60");
    assert_eq!(note(&b).position.x, 60.0);
    b.undo();
    assert_eq!(note(&b).position.x, 40.0);
}

#[test]
fn m47_a_locked_note_refuses_text_layout_and_size() {
    let (mut b, item) = note_bench();
    b.panel(PanelMsg::ToggleItemLocked(item));
    assert!(note(&b).locked);
    type_at_end(&mut b, item, "!");
    submit(&mut b, item);
    layout(&mut b, item, NoteLayout::JustifyH(HAlign::Right));
    b.enter(item, PlacedField::TextSize, "2.54");
    assert_eq!(note(&b).text, "Hello");
    assert_eq!(note(&b).justify_h, HAlign::Left);
    assert_eq!(note(&b).font_size, 1.27);
}
