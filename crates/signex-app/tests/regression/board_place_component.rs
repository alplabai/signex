//! #2505 — Place ▸ Component on a board: the row, greyed "not available on
//! boards yet" before, opens the part picker; the picked part's footprint
//! goes on the cursor, Space turns it and L (Flip Selection) moves it to the
//! other side, each click places it as one undo step, and the placed
//! footprint is not linked to a schematic part.
//!
//! The board and the real `Stock.snxlib` of `board_part_place.rs`: STOCK-D's
//! default footprint is Chip-0805-Polarised and the board holds D3.

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::library::LibraryMessage;
use signex_app::library::messages::PickerMsg;
use signex_app::pcb_canvas::CarryKind;
use signex_types::pcb::Point;

use crate::board_part_place::{Fixture, carry, click, on_grid, open, placed, point_at, text};
use crate::library_footprint_pick::key_of;
use crate::menu_support::menu_row;
use crate::settle_support::settle;

/// Place ▸ Component on the board, then pick `pn` in the picker and Place.
fn pick_from_place_menu(app: &mut Signex, pn: &str) {
    settle(app, menu_row("place_board_component"));
    assert!(
        app.library.picker.is_some(),
        "Place ▸ Component opens the picker"
    );
    assert!(
        app.interaction_state.pcb_tool.is_none(),
        "no click tool is armed"
    );
    let row = key_of(app, pn).row_id.0;
    settle(
        app,
        Message::Library(LibraryMessage::Picker(PickerMsg::SelectComponent(row))),
    );
    settle(
        app,
        Message::Library(LibraryMessage::Picker(PickerMsg::PlaceSelected)),
    );
}

#[test]
fn the_picked_part_goes_on_the_cursor_and_a_click_places_it_in_one_step() {
    let Fixture { mut app, _tmp } = open();
    let before = text(&app);
    pick_from_place_menu(&mut app, "STOCK-D");
    assert!(app.library.picker.is_none(), "the pick closes the picker");
    assert_eq!(
        carry(&app),
        Some(CarryKind::Place),
        "the part is on the cursor"
    );

    point_at(&mut app, 10.1, 20.2);
    assert!(
        app.interaction_state.pcb_canvas.has_part_ghost(),
        "the footprint follows the pointer"
    );
    click(&mut app, 10.1, 20.2);
    let first = placed(&app);
    assert_eq!(first.len(), 1, "one footprint placed");
    let d4 = &first[0];
    let at = Point::new(on_grid(10.1), on_grid(20.2));
    let on_point = (d4.position.x - at.x).abs() < 1e-9 && (d4.position.y - at.y).abs() < 1e-9;
    assert!(on_point, "{:?} on the grid", d4.position);
    assert_eq!(d4.reference, "D4", "numbered after D3");
    assert_eq!(d4.footprint_id, "Chip-0805-Polarised");
    assert_eq!(d4.layer, "TopCopper");
    assert!(
        d4.schematic_link.is_none(),
        "placed without a schematic part, it is unlinked"
    );
    assert_eq!(carry(&app), Some(CarryKind::Place), "still placing");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(text(&app), before, "one undo takes the click back");
}

#[test]
fn space_turns_and_l_flips_the_part_from_place_component() {
    let Fixture { mut app, _tmp } = open();
    pick_from_place_menu(&mut app, "STOCK-D");
    // Space on a board: a quarter turn counter-clockwise.
    settle(&mut app, menu_row("rotate_counterclockwise"));
    click(&mut app, 10.0, 10.0);
    let turned = placed(&app).pop().expect("placed");
    assert_eq!(turned.layer, "TopCopper");
    assert_eq!(turned.rotation, 270.0, "a quarter turn counter-clockwise");

    // L: Flip Selection moves the part on the cursor to the bottom.
    settle(&mut app, menu_row("flip_selection"));
    click(&mut app, 20.0, 10.0);
    let flipped = placed(&app).pop().expect("placed");
    assert_eq!(flipped.layer, "BottomCopper", "L put it on the bottom");
    // 270° on top, mirrored to the bottom, reads 90°.
    assert_eq!(flipped.rotation, 90.0);
    assert!(
        flipped
            .pads
            .iter()
            .all(|p| !p.layers.iter().any(|l| l.starts_with("Top"))),
        "its pads moved to the bottom too"
    );
    assert_eq!(placed(&app).len(), 2, "each click placed one");
}
