//! #1966: Delete and the drawing-order commands typed in an undocked window
//! act on that window's sheet.

use super::*;

fn press_in_window(f: &mut Fixture, id: &str) {
    let _ = f.app.update(Message::Ui(crate::app::UiMsg::MenuInWindow {
        window: f.window,
        msg: crate::menu_bar::MenuMessage::command(id),
    }));
}

#[test]
fn delete_typed_in_a_window_deletes_its_selection_when_main_has_none() {
    let mut f = fixture();
    select_wire_in(f.app.interaction_state.canvases.get_mut(&f.window).unwrap());
    assert!(f.app.interaction_state.canvas.selected.is_empty());
    press_in_window(&mut f, "delete_selection");
    assert!(
        f.app.document_state.engines[&f.window_path]
            .document()
            .wires
            .is_empty()
    );
}

#[test]
fn delete_typed_in_a_window_with_nothing_selected_there_leaves_main_alone() {
    let mut f = fixture();
    // Main shows a sheet with a wire that is selected; the window selects nothing.
    let main_engine = f.app.document_state.engines.get_mut(&f.main_path).unwrap();
    main_engine
        .execute(signex_engine::Command::PlaceWireSegment {
            wire: Wire {
                uuid: wire_id(),
                start: Point::new(10.16, 10.16),
                end: Point::new(20.32, 10.16),
                ..Default::default()
            },
        })
        .unwrap();
    select_wire_in(&mut f.app.interaction_state.canvas);
    press_in_window(&mut f, "delete_selection");
    assert_eq!(
        f.app.document_state.engines[&f.main_path]
            .document()
            .wires
            .len(),
        1,
        "main's selected wire was deleted by a command typed in the window"
    );
    assert_eq!(
        f.app.document_state.engines[&f.window_path]
            .document()
            .wires
            .len(),
        1
    );
}

#[test]
fn arranging_drawings_and_the_other_sheet_edits_from_a_window_run_in_its_swap() {
    use crate::app::{EditMsg, InputMsg, OverlayMsg};
    use crate::library::{LibraryMessage, PlacedPartAction, PlacedPartMsg};
    for arrange in [
        signex_engine::DrawingArrange::ToFront,
        signex_engine::DrawingArrange::ToBack,
        signex_engine::DrawingArrange::Forward,
        signex_engine::DrawingArrange::Backward,
    ] {
        assert!(command_edits_shown_sheet(&Message::Edit(
            EditMsg::ArrangeDrawings(arrange)
        )));
    }
    for message in [
        Message::Library(LibraryMessage::PlacedPart(PlacedPartMsg::ActionRequested(
            PlacedPartAction::EditSymbol,
        ))),
        Message::Overlay(OverlayMsg::ToggleAutoFocus),
        Message::Input(InputMsg::EscapePressed { window: None }),
    ] {
        assert!(command_edits_shown_sheet(&message), "{message:?}");
    }
}

#[test]
fn bring_to_front_typed_in_a_window_reorders_the_windows_drawings_not_mains() {
    let mut f = fixture();
    let rect = |n: u128| signex_types::schematic::SchDrawing::Line {
        uuid: uuid::Uuid::from_u128(n),
        start: Point::new(1.0, 1.0),
        end: Point::new(5.0, 5.0),
        width: 0.1,
        stroke_color: None,
        locked: false,
    };
    let first = rect(101);
    let second = rect(102);
    let order_of = |app: &Signex, path: &PathBuf| -> Vec<uuid::Uuid> {
        app.document_state.engines[path]
            .document()
            .drawings
            .iter()
            .map(|d| d.uuid())
            .collect()
    };
    let engine = f
        .app
        .document_state
        .engines
        .get_mut(&f.window_path)
        .unwrap();
    for drawing in [first, second] {
        engine
            .execute(signex_engine::Command::PlaceSchDrawing { drawing })
            .unwrap();
    }
    f.app
        .interaction_state
        .canvases
        .get_mut(&f.window)
        .unwrap()
        .selected = vec![SelectedItem::new(
        uuid::Uuid::from_u128(101),
        SelectedKind::Drawing,
    )];
    press_in_window(&mut f, "bring_drawings_to_front");
    assert_eq!(
        order_of(&f.app, &f.window_path),
        vec![uuid::Uuid::from_u128(102), uuid::Uuid::from_u128(101)]
    );
}
