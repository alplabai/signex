//! #2528 — the palette's "Clear all colours" clears the active sheet's net
//! colours as one undo step, and leaves the other open sheets alone.

use super::net_color_driver::*;
use crate::app::{Message, NetColorMsg};
use std::path::PathBuf;

fn clear_all(app: &mut crate::app::Signex) {
    let _ = app.update(Message::NetColor(NetColorMsg::ClearedAll));
}

#[test]
fn clear_all_is_one_undo_step() {
    let (mut app, path) = app_with_sheet("clear-all.snxsch");
    let wire = wire_of(&app, &path);
    let junction = app.document_state.engines[&path].document().junctions[0].uuid;
    paint(&mut app, &[wire], color(10));
    paint(&mut app, &[junction], color(20));
    app.ui_state.net_colors.insert("N1".to_string(), color(10));

    clear_all(&mut app);
    assert_eq!(colour_of(&app, wire), None);
    assert_eq!(colour_of(&app, junction), None);
    assert!(
        app.ui_state.net_colors.is_empty(),
        "the per-name record goes too, so the Active Bar offers no clear"
    );

    app.handle_undo_requested();
    assert_eq!(
        colour_of(&app, wire),
        Some(color(10)),
        "one undo restores all"
    );
    assert_eq!(colour_of(&app, junction), Some(color(20)));
}

#[test]
fn clear_all_keeps_the_other_sheets() {
    let (mut app, a) = app_with_sheet("a.snxsch");
    let a_wire = wire_of(&app, &a);
    paint(&mut app, &[a_wire], color(30));
    let b = PathBuf::from("b.snxsch");
    add_sheet(&mut app, &b);
    let b_wire = wire_of(&app, &b);
    paint(&mut app, &[b_wire], color(40));

    clear_all(&mut app);
    assert_eq!(colour_of(&app, b_wire), None, "the active sheet is cleared");
    assert_eq!(
        colour_of(&app, a_wire),
        Some(color(30)),
        "the other is kept"
    );
}

/// Nothing coloured: Clear all is no edit, so there is nothing to undo.
#[test]
fn clear_all_on_a_clean_sheet_is_no_step() {
    let (mut app, path) = app_with_sheet("clean.snxsch");
    clear_all(&mut app);
    assert!(!is_dirty(&app, &path));
}

/// The per-name record spans every sheet: a clear of one sheet keeps it
/// while another sheet still shows a colour; the clear of all drops it.
#[test]
fn a_one_sheet_clear_keeps_the_record_while_another_sheet_is_coloured() {
    let (mut app, a) = app_with_sheet("record-a.snxsch");
    let a_wire = wire_of(&app, &a);
    paint(&mut app, &[a_wire], color(30));
    let b = PathBuf::from("record-b.snxsch");
    add_sheet(&mut app, &b);
    let b_wire = wire_of(&app, &b);
    paint(&mut app, &[b_wire], color(40));
    app.ui_state.net_colors.insert("N1".to_string(), color(30));

    clear_all(&mut app);
    assert_eq!(colour_of(&app, b_wire), None);
    assert!(
        !app.ui_state.net_colors.is_empty(),
        "sheet a is still coloured"
    );

    app.run_clear_all_net_colors(None);
    assert_eq!(colour_of(&app, a_wire), None);
    assert!(
        app.ui_state.net_colors.is_empty(),
        "every sheet was cleared"
    );
}
