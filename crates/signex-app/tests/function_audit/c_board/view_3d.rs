//! Mockup row 109, the board 3D view: the 2D | 3D switch, the viewpoints with
//! the last one highlighted, Fit, a click that selects, the real layer stack
//! (decision 2026-11-21) and the model-unread toast. View state only: no
//! undo step and no dirty mark (decision 2026-11-15).

use pcb_geom::ItemRef;
use signex_app::app::{Board3dGesture, Board3dMsg, Message, PcbMsg};
use signex_app::pcb_canvas::layer_style::board_3d_colors;
use signex_gfx::camera3d::ViewPreset;
use signex_renderer::pcb::PcbLayerView;
use signex_types::part::FootprintSourceRef;
use uuid::Uuid;

use super::support::{Bench, R1, audit_board, open_board, open_sheet};

macro_rules! session {
    ($b:expr) => {
        $b.app
            .interaction_state
            .board_3d
            .as_ref()
            .expect("the board is in 3D")
    };
}

fn view(b: &mut Bench, event: Board3dMsg) {
    b.send(Message::Pcb(PcbMsg::Board3d(event)));
}

fn in_3d() -> Bench {
    let mut b = open_board(audit_board());
    b.command("view_board_3d");
    b
}

fn is_3d(b: &Bench) -> bool {
    b.app.interaction_state.board_3d.is_some()
}

fn gesture(b: &mut Bench, gesture: Board3dGesture) {
    let viewport = session!(b).viewport;
    view(b, Board3dMsg::Pointer { viewport, gesture });
}

/// A click where board point `(x, y)` at height `z` shows.
fn click_at(b: &mut Bench, x: f32, y: f32, z: f32) {
    let [width, height] = session!(b).viewport;
    let at = session!(b)
        .camera
        .project([x, -y, z], width, height)
        .expect("in front of the eye");
    gesture(b, Board3dGesture::Clicked(at));
}

fn selected(b: &Bench) -> Vec<ItemRef> {
    b.app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect()
}

#[test]
fn the_3d_command_shows_the_board_and_the_2d_segment_lays_it_flat_with_no_edit() {
    let mut b = open_board(audit_board());
    b.leaves_document(|b| {
        b.command("view_board_3d");
        assert!(is_3d(b), "the command shows the board in 3D");
        view(b, Board3dMsg::LeaveRequested);
        assert!(!is_3d(b), "the 2D segment lays it flat");
        b.command("view_board_3d");
        b.command("view_board_3d");
        assert!(!is_3d(b), "the command toggles");
    });
    assert!(!b.can_undo_board(), "the view makes no undo step");
}

#[test]
fn the_last_viewpoint_picked_stays_highlighted() {
    let mut b = in_3d();
    for preset in [
        ViewPreset::Top,
        ViewPreset::Iso,
        ViewPreset::Front,
        ViewPreset::Left,
        ViewPreset::Right,
        ViewPreset::Back,
    ] {
        view(&mut b, Board3dMsg::PresetPicked(preset));
        assert_eq!(session!(b).active_preset, Some(preset));
    }
    assert!(!b.dirty(), "a viewpoint is no edit");
}

#[test]
fn each_viewpoint_moves_the_camera() {
    let mut b = in_3d();
    view(&mut b, Board3dMsg::PresetPicked(ViewPreset::Top));
    let top = session!(b).camera;
    view(&mut b, Board3dMsg::PresetPicked(ViewPreset::Front));
    let front = session!(b).camera;
    view(&mut b, Board3dMsg::PresetPicked(ViewPreset::Left));
    let left = session!(b).camera;
    assert_ne!(top.pitch_deg, front.pitch_deg, "Front looks level");
    assert_ne!(front.yaw_deg, left.yaw_deg, "Left looks from its edge");
}

#[test]
fn turning_the_view_by_hand_clears_the_highlighted_viewpoint() {
    let mut b = in_3d();
    view(&mut b, Board3dMsg::PresetPicked(ViewPreset::Iso));
    let before = session!(b).camera;
    gesture(&mut b, Board3dGesture::Orbited { dx: 30.0, dy: 10.0 });
    assert_ne!(
        session!(b).camera.yaw_deg,
        before.yaw_deg,
        "the view turned"
    );
    assert_eq!(session!(b).active_preset, None);
}

#[test]
fn fit_frames_the_board_again_from_where_it_is_seen() {
    let mut b = in_3d();
    view(&mut b, Board3dMsg::PresetPicked(ViewPreset::Iso));
    let framed = session!(b).camera;
    gesture(&mut b, Board3dGesture::Zoomed(-5.0));
    let zoomed_out = session!(b).camera;
    assert!(zoomed_out.distance > framed.distance, "the wheel zooms out");
    view(&mut b, Board3dMsg::FitRequested);
    let fitted = session!(b).camera;
    assert!(fitted.distance < zoomed_out.distance, "Fit comes back");
    assert_eq!(fitted.yaw_deg, framed.yaw_deg, "Fit keeps the direction");
    assert_eq!(fitted.pitch_deg, framed.pitch_deg);
}

#[test]
fn a_click_on_a_part_selects_it_and_a_click_on_the_bare_board_clears_it() {
    let mut b = in_3d();
    // The board is 1.6 mm thick and a part with no height stands 1 mm tall.
    click_at(&mut b, 5.0, 5.0, 2.6);
    assert_eq!(selected(&b), vec![ItemRef::Footprint(R1)]);
    click_at(&mut b, 30.0, 27.0, 1.6);
    assert!(
        selected(&b).is_empty(),
        "the bare board clears the selection"
    );
    assert!(!b.dirty(), "a selection is no edit");
    assert!(!b.can_undo_board());
}

#[test]
fn the_stack_runs_from_copper_finish_through_mask_to_silkscreen() {
    let b = in_3d();
    let board = b.board();
    let colors = board_3d_colors();
    let built = session!(b).scene(&board, 0, &PcbLayerView::default(), colors);
    let highest = |color: [f32; 4]| {
        built
            .scene
            .scene
            .vertices
            .iter()
            .filter(|v| v.color == color)
            .map(|v| v.position[2])
            .fold(f32::MIN, f32::max)
    };
    let (finish, mask, silk) = (
        highest(colors.finish),
        highest(colors.mask),
        highest(colors.silk),
    );
    assert!(finish > f32::MIN, "exposed copper takes the finish");
    assert!(silk > f32::MIN, "the silkscreen line is drawn");
    assert!(finish < mask, "the mask lies over the copper");
    assert!(mask < silk, "the silkscreen lies over the mask");
}

#[test]
fn a_model_that_cannot_be_read_shows_a_toast_that_dismisses_and_messages_keeps_it() {
    let mut board = audit_board();
    board.footprints[0].library_ref = Some(FootprintSourceRef {
        library: Uuid::from_u128(0xC77),
        footprint: Uuid::from_u128(0xC78),
        version: "1".into(),
    });
    let mut b = open_board(board);
    b.command("view_board_3d");
    let (source, _why) = session!(b)
        .unread
        .first()
        .cloned()
        .expect("the part's model could not be read");
    view(&mut b, Board3dMsg::UnreadNoticeDismissed);
    assert!(session!(b).unread.is_empty(), "the toast is gone");
    let kept = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|entry| entry.message.contains(&source));
    assert!(kept, "Messages keeps the line");
    assert!(!b.dirty());
}

#[test]
fn the_3d_view_is_refused_on_a_sheet() {
    let mut b = open_sheet();
    b.leaves_document(|b| view(b, Board3dMsg::Toggled));
    assert!(!is_3d(&b), "a sheet has no 3D view");
}
