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

/// #2744: the second paste with the defaults would call its net D0_2 too. The
/// dialog names it, Paste stops with nothing on the cursor, and another Start
/// lets it through.
#[test]
fn a_rewritten_name_that_exists_stops_the_paste_and_is_named() {
    let mut app = app_with_copy();
    smart_paste_dialog(&mut app, |_| {});
    click(&mut app, (60.96, 60.96));
    assert!(label_texts(&app).iter().any(|t| t == "D0_2"));
    let before = label_texts(&app);
    smart_paste_dialog(&mut app, |app| {
        let state = app.ui_state.smart_paste.as_ref().expect("open");
        assert_eq!(state.taken_names(), ["D0_2"]);
        assert!(!state.can_paste());
    });
    assert!(
        app.interaction_state.placement.is_none(),
        "nothing goes on the cursor"
    );
    assert!(app.ui_state.smart_paste.is_some(), "the dialog stays open");
    assert_eq!(label_texts(&app), before);
    dialog(&mut app, SmartPasteMsg::StartEdited("3".into()));
    let state = app.ui_state.smart_paste.as_ref().expect("open");
    assert!(state.can_paste(), "D0_3 is free");
    dialog(&mut app, SmartPasteMsg::PasteConfirmed);
    assert!(app.interaction_state.placement.is_some());
}

/// The project read for this dialog could not read `bad.snxsch`.
fn with_an_unread_sheet(app: &mut Signex) -> std::path::PathBuf {
    let bad = std::path::PathBuf::from("bad.snxsch");
    let target = app.document_state.active_path.clone().expect("a sheet");
    app.interaction_state.paste_unread_sheets = Some((target, vec![bad.clone()]));
    bad
}

/// #2744: a rewritten name may join a net on a sheet that was not read, so
/// Paste is off with the sheet named, and a direct Paste says so in Messages
/// and leaves the dialog open with nothing on the cursor.
#[test]
fn an_unread_sheet_stops_a_paste_that_rewrites_a_net_and_is_named() {
    let _ = signex_app::diagnostics::init_logging();
    let before = signex_app::diagnostics::newest_entry_id();
    let mut app = app_with_copy();
    let labels = label_texts(&app);
    smart_paste_dialog(&mut app, |app| {
        let bad = with_an_unread_sheet(app);
        dialog(app, SmartPasteMsg::NetsPicked(NetChoice::Rewrite));
        let state = app.ui_state.smart_paste.as_ref().expect("open");
        assert_eq!(state.unread_sheets_in_the_way(), [bad]);
        assert!(!state.can_paste());
    });
    assert!(
        app.interaction_state.placement.is_none(),
        "nothing goes on the cursor"
    );
    assert!(app.ui_state.smart_paste.is_some(), "the dialog stays open");
    assert_eq!(label_texts(&app), labels);
    assert!(
        signex_app::diagnostics::entries_since(before)
            .entries
            .iter()
            .any(|e| e.message.contains("Smart Paste stopped") && e.message.contains("bad.snxsch")),
        "Messages names the sheet"
    );
}

/// #2744: Keep rewrites no net name, so it has nothing to collide with.
#[test]
fn an_unread_sheet_does_not_stop_a_paste_that_keeps_its_nets() {
    let mut app = app_with_copy();
    smart_paste_dialog(&mut app, |app| {
        with_an_unread_sheet(app);
        dialog(app, SmartPasteMsg::NetsPicked(NetChoice::Keep));
        let state = app.ui_state.smart_paste.as_ref().expect("open");
        assert!(state.can_paste());
    });
    assert!(app.interaction_state.placement.is_some());
}

/// #2744: the headless and agent `smart_paste` places labels as copied. It
/// rewrites no net name, so by the rule an unread sheet does not stop it.
#[test]
fn a_headless_smart_paste_rewrites_no_net_so_an_unread_sheet_does_not_stop_it() {
    let mut app = app_with_copy();
    app.ui_state.io = signex_app::io_ports::IoPorts::headless(Default::default());
    let before = label_texts(&app).len();
    let _ = app.update(Message::Edit(EditMsg::SmartPaste));
    with_an_unread_sheet(&mut app);

    crate::paste_support::answer(&mut app, signex_app::app::PasteIntent::SmartPaste, None);

    let texts = label_texts(&app);
    assert_eq!(texts.len(), before * 2, "the copy landed: {texts:?}");
    assert_eq!(
        texts[before..],
        texts[..before],
        "no net name was rewritten"
    );
    assert!(app.ui_state.smart_paste.is_none());
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
