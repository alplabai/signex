//! Mockup row 155 - Smart Paste in a window: the dialog's offset or array, the
//! designator choice and the net-name pattern decide what lands; power nets
//! are never rewritten; Paste puts every copy on the cursor and one click
//! places them as one undo step.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::smart_paste::{NetChoice, PlaceMode, SmartPasteMsg};
use signex_types::schematic::{Label, LabelType, Point};

use crate::paste_support::sheet::{app_with_sheet, click, populated_sheet, select};
use crate::paste_support::smart_paste_dialog;

fn dialog(app: &mut Signex, msg: SmartPasteMsg) {
    let _ = app.update(Message::Edit(EditMsg::SmartPasteDialog(msg)));
}

/// R1 and the labels D0 (a signal net) and GND (a power net), copied.
fn app_with_copy() -> Signex {
    let mut sheet = populated_sheet();
    sheet.labels.push(Label::new(
        "GND",
        Point::new(10.16, 20.32),
        LabelType::Power,
    ));
    let mut app = app_with_sheet(sheet);
    select(&mut app, &[("R1", 1)], true);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    app
}

fn label_texts(app: &Signex) -> Vec<String> {
    let doc = app.document_state.active_engine().unwrap().document();
    doc.labels.iter().map(|l| l.text.clone()).collect()
}

#[test]
fn an_array_with_a_pattern_renames_signal_nets_and_keeps_power_nets() {
    let mut app = app_with_copy();
    let steps = app.document_state.active_engine().unwrap().undo_position();
    smart_paste_dialog(&mut app, |app| {
        dialog(app, SmartPasteMsg::ModePicked(PlaceMode::Array));
        dialog(app, SmartPasteMsg::CopiesEdited("2".into()));
        dialog(app, SmartPasteMsg::NetsPicked(NetChoice::Rewrite));
        let state = app.ui_state.smart_paste.as_ref().expect("open");
        assert_eq!(state.copy_count(), 2);
        assert!(
            state.next_free_text.contains("R1 \u{2192} R4, R5"),
            "the designator line lists every copy: {}",
            state.next_free_text
        );
    });
    click(&mut app, (60.96, 60.96));
    let texts = label_texts(&app);
    assert_eq!(&texts[..2], ["D0", "GND"], "the originals stay");
    let mut pasted: Vec<String> = texts[2..].to_vec();
    pasted.sort();
    assert_eq!(
        pasted,
        ["D0_2", "D0_3", "GND", "GND"],
        "power nets are kept"
    );
    assert_eq!(
        app.document_state.active_engine().unwrap().undo_position(),
        steps + 1,
        "every copy lands as one undo step"
    );
}

#[test]
fn keep_lets_the_pasted_nets_join_the_originals() {
    let mut app = app_with_copy();
    smart_paste_dialog(&mut app, |app| {
        dialog(app, SmartPasteMsg::NetsPicked(NetChoice::Keep));
    });
    click(&mut app, (60.96, 60.96));
    let texts = label_texts(&app);
    assert_eq!(texts.iter().filter(|t| *t == "D0").count(), 2);
}

#[test]
fn the_offset_puts_the_copy_beside_the_click() {
    let mut app = app_with_copy();
    smart_paste_dialog(&mut app, |app| {
        dialog(app, SmartPasteMsg::OffsetXEdited("10.16".into()));
        dialog(app, SmartPasteMsg::OffsetYEdited("0".into()));
    });
    let session = app
        .interaction_state
        .placement
        .as_ref()
        .expect("the copy is on the cursor");
    let ghost_x = session.payload.fragment().symbols[0].position.x;
    let source_x = app
        .interaction_state
        .clipboard
        .as_ref()
        .unwrap()
        .payload
        .fragment()
        .symbols[0]
        .position
        .x;
    assert!((ghost_x - (source_x + 10.16)).abs() < 1e-9);
}

#[test]
fn cancel_closes_the_dialog_and_pastes_nothing() {
    let mut app = app_with_copy();
    let count = app
        .document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols
        .len();
    let task = app.update(Message::Edit(EditMsg::SmartPaste));
    crate::paste_support::settle(&mut app, task, true);
    crate::paste_support::answer(&mut app, signex_app::app::PasteIntent::SmartPaste, None);
    assert!(app.ui_state.smart_paste.is_some());
    dialog(&mut app, SmartPasteMsg::Cancelled);
    assert!(app.ui_state.smart_paste.is_none());
    assert!(app.interaction_state.placement.is_none());
    assert_eq!(
        app.document_state
            .active_engine()
            .unwrap()
            .document()
            .symbols
            .len(),
        count
    );
}
