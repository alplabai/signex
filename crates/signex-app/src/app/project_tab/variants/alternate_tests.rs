//! Mockup rows 32 and 208: Alternate… in the Variant Manager's right pane
//! opens the canvas's Alternate part picker for the cell's part: a plain
//! part on its first placed unit, a channel's copy with that channel shown
//! and the scope offered. Base's cell opens nothing.

use super::manager::VariantManagerMsg;
use super::manager_tests::{app, cell, manage, open_manager};
use super::part_commands_tests::repeated;
use crate::app::variant_part_edit::VariantPartEdit;

/// The open picker's part.
macro_rules! picked_part {
    ($app:expr) => {
        match $app.ui_state.variant_part_edit.as_deref() {
            Some(VariantPartEdit::Alternate(picker)) => &picker.part,
            other => panic!("the picker is open: {other:?}"),
        }
    };
}

#[test]
fn alternate_opens_the_picker_on_the_parts_first_placed_unit() {
    let (mut app, id, _dir) = app();
    open_manager(&mut app, id);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::AlternateRequested {
            cell: cell(None, "R1"),
        },
    );
    assert!(
        app.ui_state.variant_part_edit.is_none(),
        "Base opens nothing"
    );

    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::AlternateRequested {
            cell: cell(Some("Lite"), "R1"),
        },
    );
    let part = picked_part!(app);
    assert_eq!(part.variant, "Lite");
    assert_eq!(part.this_part.designator, "R1");
    assert_eq!(part.base.value, "1k");
    assert_eq!(part.sheet, "a.snxsch");
    assert_eq!(part.channel, None);
    assert!(!part.has_scope());
    assert_eq!(part.window, None, "the main window shows it");
}

#[test]
fn a_channels_copy_opens_with_that_channel_and_the_scope() {
    let (mut app, id, _dir) = repeated();
    open_manager(&mut app, id);
    let _ = manage(
        &mut app,
        id,
        VariantManagerMsg::AlternateRequested {
            cell: cell(Some("Lite"), "CH2_R5"),
        },
    );
    let part = picked_part!(app);
    assert_eq!(part.this_part.designator, "CH2_R5");
    assert_eq!(part.channel.as_deref(), Some("CH2"));
    assert_eq!(part.channels, Some(("CH1".to_string(), "CH3".to_string())));
    let copies: Vec<&str> = part
        .every_channel
        .iter()
        .map(|copy| copy.designator.as_str())
        .collect();
    assert_eq!(copies, ["CH1_R5", "CH2_R5", "CH3_R5"]);
    assert_eq!(part.sheet, "child.snxsch");
    assert_eq!(part.base.value, "31.6k");
    assert!(part.has_scope());
}
