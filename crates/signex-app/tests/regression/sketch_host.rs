//! #2518 — the board sketch is a host of the shared sketch toolkit: its
//! role picker lists the board's bake targets, V-score is set from it,
//! Bake commits one undoable board command and leaves, and Discard changes
//! asks before it drops edits. Driven through `Signex::update`.

use signex_app::app::{BoardSketchMsg, EditMsg, InputMsg, Message, PcbMsg};
use signex_app::library::messages::RoleTag;
use signex_app::panels::PanelMsg;
use signex_types::sketch::SketchHost;

use crate::board_sketch::{Fixture, in_sketch, live, open_board, panel, rectangle, send, snapshot};
use crate::menu_support::menu_row;

fn sketch(f: &mut Fixture, event: BoardSketchMsg) {
    send(f, Message::Pcb(PcbMsg::Sketch(event)));
}

fn entity(f: &Fixture, id: signex_sketch::id::SketchEntityId) -> signex_sketch::entity::Entity {
    let session = f.app.interaction_state.board_sketch.as_ref().expect("on");
    session
        .working_sketch()
        .entities
        .into_iter()
        .find(|e| e.id == id)
        .expect("the entity")
}

#[test]
fn the_board_host_offers_its_targets_and_v_score_bakes_as_one_step() {
    let mut f = open_board();
    let before = snapshot(&f);
    send(&mut f, menu_row("board_sketch"));
    let ctx = f
        .app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .expect("the sketch drives Properties");
    assert_eq!(ctx.sketch_host, SketchHost::Board);
    assert_eq!(
        ctx.sketch_host.roles(),
        // Decision 2 (Caner, 2026-11-20): the mockup 179 order, Room out.
        &[
            RoleTag::Unassigned,
            RoleTag::BoardOutline,
            RoleTag::Cutout,
            RoleTag::Slot,
            RoleTag::Keepout,
            RoleTag::BoardPour,
            RoleTag::VScore,
            RoleTag::MechanicalLayer,
            RoleTag::ComponentAnchor,
            RoleTag::MountingHole,
        ]
    );

    let edges = rectangle(&mut f, (0.0, 0.0), (40.0, 30.0));
    for &edge in &edges {
        panel(
            &mut f,
            PanelMsg::FpEditorSetRole {
                id: edge,
                role: RoleTag::BoardOutline,
            },
        );
    }
    // Apart from the outline, so no snap joins the two profiles.
    let score = rectangle(&mut f, (60.0, 0.0), (70.0, 10.0))[0];
    panel(
        &mut f,
        PanelMsg::FpEditorSetRole {
            id: score,
            role: RoleTag::VScore,
        },
    );
    assert!(
        entity(&f, score).v_score.is_some(),
        "the role picker sets V-score"
    );
    assert!(entity(&f, edges[0]).board_outline);
    assert_eq!(
        snapshot(&f),
        before,
        "nothing reaches the board while drawing"
    );

    sketch(&mut f, BoardSketchMsg::BakeRequested);
    assert!(!in_sketch(&f), "Bake applies and leaves");
    let after = live(&f);
    assert_eq!(after.outline.len(), 4, "the outline reached the board");
    assert_eq!(after.sketch.as_ref().expect("stored").v_scores.len(), 1);

    send(&mut f, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f), before, "Bake is one undo step");
}

#[test]
fn discard_changes_asks_first_and_restores_the_board() {
    let mut f = open_board();
    let before = snapshot(&f);
    send(&mut f, menu_row("board_sketch"));
    rectangle(&mut f, (0.0, 0.0), (40.0, 30.0));

    // Esc with nothing in flight is Discard changes, and it asks.
    send(
        &mut f,
        Message::Input(InputMsg::EscapePressed { window: None }),
    );
    assert!(in_sketch(&f), "edits are not dropped without asking");
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert!(session.editor.state.sketch_exit.confirming_discard);

    // A second Esc answers Keep editing.
    send(
        &mut f,
        Message::Input(InputMsg::EscapePressed { window: None }),
    );
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert!(!session.editor.state.sketch_exit.confirming_discard);
    assert!(session.has_changes(), "the edits are kept");

    sketch(&mut f, BoardSketchMsg::DiscardRequested);
    sketch(&mut f, BoardSketchMsg::DiscardConfirmed);
    assert!(!in_sketch(&f));
    assert_eq!(
        snapshot(&f),
        before,
        "the board is as it was before the sketch"
    );
}

#[test]
fn bake_preview_counts_the_board_targets_without_touching_the_board() {
    let mut f = open_board();
    let before = snapshot(&f);
    send(&mut f, menu_row("board_sketch"));
    let edges = rectangle(&mut f, (0.0, 0.0), (40.0, 30.0));
    panel(
        &mut f,
        PanelMsg::FpEditorSetRole {
            id: edges[0],
            role: RoleTag::BoardOutline,
        },
    );
    sketch(&mut f, BoardSketchMsg::BakePreviewToggled);
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    use signex_app::library::editor::footprint::sketch_mode::BakePreview;
    use signex_types::sketch::baked_kinds::BakedKind;
    let Some(BakePreview::Counts(counts)) = session.preview.as_ref() else {
        panic!("the preview bakes: {:?}", session.preview);
    };
    assert_eq!(
        counts.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(),
        SketchHost::Board.bake_targets()
    );
    assert!(counts.contains(&(BakedKind::BoardOutline, 1)));
    assert_eq!(snapshot(&f), before, "a preview writes nothing");

    sketch(&mut f, BoardSketchMsg::BakePreviewToggled);
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert!(session.preview.is_none(), "Back to sketch closes it");
}
