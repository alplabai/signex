//! #2527 — Paste Array's "Count up label numbers" off is step 0: every copy
//! keeps the copied label text, as one undo step.

use signex_app::app::{EditMsg, Message};
use signex_app::paste_array::PasteArrayMsg;
use signex_types::coord::Unit;

use crate::paste_stamp_array::open_array;
use crate::paste_support::sheet::{app_with_sheet, populated_sheet, select};

fn dialog(app: &mut signex_app::app::Signex, msg: PasteArrayMsg) {
    let _ = app.update(Message::Edit(EditMsg::PasteArray(msg)));
}

#[test]
fn count_up_off_places_copies_that_keep_the_label_text() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.unit = Unit::Mm;
    select(&mut app, &[("R1", 1)], true);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    open_array(&mut app);

    dialog(&mut app, PasteArrayMsg::CountChanged("4".to_owned()));
    dialog(&mut app, PasteArrayMsg::CountUpToggled(false));
    let state = app
        .ui_state
        .paste_array
        .as_ref()
        .expect("the dialog is open");
    assert_eq!(state.step, "0", "off is step 0, no stored flag");
    assert!(!state.counts_up());
    dialog(&mut app, PasteArrayMsg::ApplyPressed);
    assert!(app.ui_state.paste_array.is_none(), "the paste closes it");

    let doc = app.document_state.active_engine().unwrap().document();
    let labels: Vec<_> = doc.labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(labels, ["D0", "D0", "D0", "D0"], "every copy keeps D0");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    let doc = app.document_state.active_engine().unwrap().document();
    assert_eq!(doc.labels.len(), 1, "one undo takes the array back");
}

#[test]
fn ticking_count_up_again_restores_a_counting_step() {
    let mut app = app_with_sheet(populated_sheet());
    app.ui_state.increment_step = 2;
    select(&mut app, &[("R1", 1)], true);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    open_array(&mut app);

    dialog(&mut app, PasteArrayMsg::CountUpToggled(false));
    dialog(&mut app, PasteArrayMsg::CountUpToggled(true));
    let state = app.ui_state.paste_array.as_ref().unwrap();
    assert_eq!(state.step, "2", "the preference step comes back");
    assert_eq!(state.sample_label.as_deref(), Some("D0"));
}
