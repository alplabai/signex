//! Mockup rows 179 and 179-slot (DECISIONS 2026-11-24, #2843), driven
//! through `Signex::update` on a real board sketch session: a Bake as pick
//! in the open Bake panel tags every edge of its profile in one undo step
//! of the sketch and leaves the board alone until Bake; a selected slot
//! edge fills the Slot header's size in the board context.

use signex_app::app::{BoardSketchMsg, EditMsg, Message, PcbMsg};
use signex_app::library::editor::footprint::SketchTool;
use signex_app::library::messages::{FootprintEditorMsg, RoleTag};
use signex_app::panels::{BoardSketchProps, PanelMsg, SketchFixMsg};
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;

use crate::board_sketch::{
    Fixture, click, edit, ids_of, in_sketch, open_board, panel, rectangle, send, snapshot,
};
use crate::menu_support::menu_row;

/// A board in sketch mode with a 20 x 10 rectangle and a slot drawn by the
/// Slot tool: centres (30, 5) and (40, 5), width click 2 mm off the centre
/// line, so 14 x 4. Returns the rectangle's lines and the slot's edges.
fn board_with_rectangle_and_slot() -> (Fixture, Vec<SketchEntityId>, Vec<SketchEntityId>) {
    let mut f = open_board();
    send(&mut f, menu_row("board_sketch"));
    assert!(in_sketch(&f), "sketch mode");
    let rect = rectangle(&mut f, (0.0, 0.0), (20.0, 10.0));
    assert_eq!(rect.len(), 4);
    let before = ids_of(&f, is_edge);
    edit(&mut f, FootprintEditorMsg::SketchSetTool(SketchTool::Slot));
    click(&mut f, 30.0, 5.0);
    click(&mut f, 40.0, 5.0);
    click(&mut f, 35.0, 7.0);
    edit(&mut f, FootprintEditorMsg::ToolEscape);
    let slot: Vec<_> = ids_of(&f, is_edge)
        .into_iter()
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(slot.len(), 4, "two lines and two arcs");
    (f, rect, slot)
}

fn is_edge(kind: &EntityKind) -> bool {
    matches!(kind, EntityKind::Line { .. } | EntityKind::Arc { .. })
}

fn sketch(f: &mut Fixture, event: BoardSketchMsg) {
    send(f, Message::Pcb(PcbMsg::Sketch(event)));
}

/// The board context Properties shows for the sketch in view.
fn board_props(f: &Fixture) -> BoardSketchProps {
    f.app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .and_then(|fp| fp.board.clone())
        .expect("board sketch Properties")
}

fn undo_len(f: &Fixture) -> usize {
    let session = f.app.interaction_state.board_sketch.as_ref();
    session.expect("sketch mode").editor.history.undo_len()
}

/// Whether each of `edges` carries a keepout, in the working sketch.
fn keepouts(f: &Fixture, edges: &[SketchEntityId]) -> Vec<bool> {
    let session = f.app.interaction_state.board_sketch.as_ref();
    let sketch = session.expect("sketch mode").working_sketch();
    edges
        .iter()
        .map(|id| {
            let edge = sketch.entities.iter().find(|e| e.id == *id);
            edge.expect("the edge is in the sketch").keepout.is_some()
        })
        .collect()
}

#[test]
fn a_bake_as_pick_in_the_bake_panel_tags_the_profile_in_one_undo_step() {
    let (mut f, rect, _) = board_with_rectangle_and_slot();
    sketch(&mut f, BoardSketchMsg::BakePreviewToggled);
    assert!(board_props(&f).bake.is_some(), "the Bake panel is open");

    let fp = f.app.document_state.panel_ctx.footprint_editor.clone();
    let rows = fp.expect("Properties").profile_roles.clone();
    assert_eq!(rows.len(), 2, "each closed profile is listed: {rows:#?}");
    let row = rows
        .iter()
        .find(|row| rect.contains(&row.seed))
        .expect("the rectangle's row");
    assert!(row.options.contains(&RoleTag::Keepout));
    assert!(
        !row.options.contains(&RoleTag::Slot),
        "a rectangle is not offered Slot"
    );

    let board_before = snapshot(&f);
    let steps = undo_len(&f);
    assert_eq!(keepouts(&f, &rect), [false; 4]);

    panel(
        &mut f,
        PanelMsg::SketchFix(SketchFixMsg::ProfileRolePicked {
            seed: row.seed,
            role: RoleTag::Keepout,
        }),
    );

    assert_eq!(keepouts(&f, &rect), [true; 4], "every edge is a keepout");
    assert_eq!(undo_len(&f), steps + 1, "one undo step");
    assert_eq!(snapshot(&f), board_before, "the board waits for Bake");

    send(&mut f, Message::Edit(EditMsg::Undo));
    assert_eq!(keepouts(&f, &rect), [false; 4], "one undo restores them");
    assert_eq!(undo_len(&f), steps);
    assert_eq!(snapshot(&f), board_before);
}

#[test]
fn selecting_a_slot_edge_fills_the_slot_size_and_keeps_the_selection() {
    let (mut f, rect, slot) = board_with_rectangle_and_slot();
    for &edge in &slot {
        edit(
            &mut f,
            FootprintEditorMsg::SketchSelect {
                id: Some(edge),
                shift: false,
            },
        );
        let session = f.app.interaction_state.board_sketch.as_ref();
        let selected = session.expect("sketch mode").editor.state.selected_sketch;
        assert_eq!(selected, Some(edge), "the selection stays the edge");
        let (length, width) = board_props(&f)
            .selected_slot
            .unwrap_or_else(|| panic!("{edge:?} reads its slot"));
        assert!(
            (length - 14.0).abs() < 1e-6 && (width - 4.0).abs() < 1e-6,
            "14 x 4, got {length} x {width}"
        );
    }
    edit(
        &mut f,
        FootprintEditorMsg::SketchSelect {
            id: Some(rect[0]),
            shift: false,
        },
    );
    assert_eq!(
        board_props(&f).selected_slot,
        None,
        "a rectangle is no slot"
    );
}
