//! Mockup row 155, Smart Paste: the dialog's offset or array, designators and
//! net-name pattern decide what lands; Cancel and a bad value paste nothing.

use signex_app::app::{EditMsg, Message, PasteIntent, PasteRequest};
use signex_app::smart_paste::{NetChoice, PlaceMode, SmartPasteMsg};
use signex_types::schematic::{Label, LabelType, Point, SelectedItem, SelectedKind};

use super::library_place_symbol::run;
use super::support::{Bench, bench, empty_sheet, net_label, part};

/// R1, R2, R3 and the labels D0 (signal) and GND (power), with R1 and both
/// labels copied.
fn copied() -> Bench {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![
        part("R1", (10.16, 10.16)),
        part("R2", (20.32, 10.16)),
        part("R3", (30.48, 10.16)),
    ];
    sheet.labels = vec![
        net_label("D0", (40.64, 10.16)),
        Label::new("GND", Point::new(10.16, 20.32), LabelType::Power),
    ];
    let mut b = bench(sheet);
    let mut items = vec![SelectedItem::new(
        b.sheet().symbols[0].uuid,
        SelectedKind::Symbol,
    )];
    items.extend(
        b.sheet()
            .labels
            .iter()
            .map(|l| SelectedItem::new(l.uuid, SelectedKind::Label)),
    );
    b.select(items);
    b.post(Message::Edit(EditMsg::Copy));
    b
}

/// Edit ▸ Smart Paste with nothing on the system clipboard: the numbering
/// read, then the clipboard's empty answer, open the dialog.
fn open_dialog(b: &mut Bench) {
    let task = b.app.update(Message::Edit(EditMsg::SmartPaste));
    for message in run(task) {
        if matches!(message, Message::Edit(EditMsg::PasteCountersSeeded { .. })) {
            let _ = b.app.update(message);
        }
    }
    let request = PasteRequest {
        intent: PasteIntent::SmartPaste,
        epoch: b.app.interaction_state.placement_epoch,
        path: b.app.document_state.active_path.clone(),
        dialog: 0,
        from_agent: false,
    };
    b.post(Message::Edit(EditMsg::PasteTextRead {
        request,
        text: None,
    }));
    assert!(b.app.ui_state.smart_paste.is_some(), "the dialog opens");
}

fn dialog(b: &mut Bench, msg: SmartPasteMsg) {
    b.post(Message::Edit(EditMsg::SmartPasteDialog(msg)));
}

fn labels(b: &Bench) -> Vec<String> {
    b.sheet().labels.iter().map(|l| l.text.clone()).collect()
}

fn references(b: &Bench) -> Vec<String> {
    b.sheet()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

#[test]
fn m155_an_array_lands_every_copy_as_one_undo_step_and_undo_removes_them() {
    let mut b = copied();
    let before = b.sheet().clone();
    open_dialog(&mut b);
    dialog(&mut b, SmartPasteMsg::ModePicked(PlaceMode::Array));
    dialog(&mut b, SmartPasteMsg::CopiesEdited("2".to_string()));
    dialog(&mut b, SmartPasteMsg::NetsPicked(NetChoice::Rewrite));
    dialog(&mut b, SmartPasteMsg::PasteConfirmed);
    assert!(
        b.app.ui_state.smart_paste.is_none(),
        "Paste closes the dialog"
    );
    assert_eq!(b.sheet().symbols.len(), 3, "nothing lands before the click");

    let steps = b.steps();
    b.click(60.96, 60.96);
    assert_eq!(b.sheet().symbols.len(), 5);
    assert_eq!(b.steps(), steps + 1, "one undo step");
    assert!(b.dirty());
    b.undo();
    assert_eq!(b.sheet().symbols.len(), before.symbols.len());
    assert_eq!(b.sheet().labels.len(), before.labels.len());
    assert!(!b.dirty());
}

#[test]
fn m155_the_pattern_renames_signal_nets_and_never_a_power_net() {
    let mut b = copied();
    open_dialog(&mut b);
    dialog(&mut b, SmartPasteMsg::ModePicked(PlaceMode::Array));
    dialog(&mut b, SmartPasteMsg::CopiesEdited("2".to_string()));
    dialog(&mut b, SmartPasteMsg::NetsPicked(NetChoice::Rewrite));
    dialog(&mut b, SmartPasteMsg::PasteConfirmed);
    b.click(60.96, 60.96);
    let all = labels(&b);
    let mut pasted = all[2..].to_vec();
    pasted.sort();
    assert_eq!(pasted, ["D0_2", "D0_3", "GND", "GND"]);
}

#[test]
fn m155_keep_lets_the_pasted_net_join_the_original() {
    let mut b = copied();
    open_dialog(&mut b);
    dialog(&mut b, SmartPasteMsg::NetsPicked(NetChoice::Keep));
    dialog(&mut b, SmartPasteMsg::PasteConfirmed);
    b.click(60.96, 60.96);
    assert_eq!(labels(&b).iter().filter(|t| *t == "D0").count(), 2);
}

#[test]
fn m155_next_free_continues_after_the_highest_designator() {
    let mut b = copied();
    open_dialog(&mut b);
    dialog(&mut b, SmartPasteMsg::PasteConfirmed);
    b.click(60.96, 60.96);
    assert!(
        references(&b).iter().any(|r| r == "R4"),
        "{:?}",
        references(&b)
    );
}

#[test]
fn m155_the_offset_puts_the_copy_that_far_from_the_source() {
    let mut b = copied();
    open_dialog(&mut b);
    dialog(&mut b, SmartPasteMsg::OffsetXEdited("10.16".to_string()));
    dialog(&mut b, SmartPasteMsg::OffsetYEdited("0".to_string()));
    dialog(&mut b, SmartPasteMsg::PasteConfirmed);
    let session = b
        .app
        .interaction_state
        .placement
        .as_ref()
        .expect("the copy is on the cursor");
    let ghost_x = session.payload.fragment().symbols[0].position.x;
    let source_x = b
        .app
        .interaction_state
        .clipboard
        .as_ref()
        .expect("the copy")
        .payload
        .fragment()
        .symbols[0]
        .position
        .x;
    assert!((ghost_x - (source_x + 10.16)).abs() < 1e-9);
}

#[test]
fn m155_cancel_closes_the_dialog_and_pastes_nothing() {
    let mut b = copied();
    open_dialog(&mut b);
    dialog(&mut b, SmartPasteMsg::OffsetXEdited("25.4".to_string()));
    dialog(&mut b, SmartPasteMsg::Cancelled);
    assert!(b.app.ui_state.smart_paste.is_none());
    assert!(b.app.interaction_state.placement.is_none());
    assert_eq!(b.sheet().symbols.len(), 3);
    assert!(!b.dirty());
}

#[test]
fn m155_a_bad_copy_count_keeps_the_dialog_and_the_text_and_pastes_nothing() {
    let mut b = copied();
    open_dialog(&mut b);
    dialog(&mut b, SmartPasteMsg::ModePicked(PlaceMode::Array));
    for bad in ["0", "many", "65"] {
        dialog(&mut b, SmartPasteMsg::CopiesEdited(bad.to_string()));
        dialog(&mut b, SmartPasteMsg::PasteConfirmed);
        let state = b.app.ui_state.smart_paste.as_ref().expect("stays open");
        assert_eq!(state.copies, bad, "the box keeps it");
        assert!(b.app.interaction_state.placement.is_none(), "{bad:?}");
    }
    assert!(!b.dirty());
}

#[test]
fn m155_a_pattern_that_would_not_differ_is_refused() {
    let mut b = copied();
    open_dialog(&mut b);
    dialog(&mut b, SmartPasteMsg::NetsPicked(NetChoice::Rewrite));
    dialog(&mut b, SmartPasteMsg::PatternEdited("plain".to_string()));
    dialog(&mut b, SmartPasteMsg::PasteConfirmed);
    let state = b.app.ui_state.smart_paste.as_ref().expect("stays open");
    assert_eq!(state.pattern, "plain");
    assert!(b.app.interaction_state.placement.is_none());
}
