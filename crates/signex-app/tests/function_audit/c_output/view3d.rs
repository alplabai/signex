//! Row m175: the board 3D view's toolbar. Everything it does is view state:
//! no board change, no undo step, no dirty mark.

use std::time::{Duration, Instant};

use signex_app::app::{Board3dGesture, Board3dMsg, InputMsg, Message, PcbMsg, TabDocument};
use signex_gfx::camera3d::{FLIP_DURATION_MS, LightingPreset, OrbitCamera, ViewPreset, ViewSide};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::support::{BoardBench, command, open_board};
use crate::settle_support::settle;

/// A 40 x 30 mm board with one part.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xC175),
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 40.0, "y": 0.0},
            {"x": 40.0, "y": 30.0}, {"x": 0.0, "y": 30.0}
        ],
        "layers": [
            {"id": 0, "name": "TopCopper", "layer_type": "signal"},
            {"id": 31, "name": "BottomCopper", "layer_type": "signal"}
        ],
        "footprints": [{
            "uuid": Uuid::from_u128(0xC176), "reference": "U1",
            "position": {"x": 12.0, "y": 12.0}, "layer": "TopCopper"
        }]
    }))
    .expect("the audit board")
}

fn in_3d() -> BoardBench {
    let mut b = open_board(board());
    command(&mut b.app, "view_board_3d");
    assert!(b.app.interaction_state.board_3d.is_some(), "the view is on");
    b
}

/// Send a toolbar event; a lighting toast's timer task is dropped with it.
fn send(b: &mut BoardBench, event: Board3dMsg) {
    let _ = b.app.update(Message::Pcb(PcbMsg::Board3d(event)));
}

macro_rules! session {
    ($b:expr) => {
        $b.app
            .interaction_state
            .board_3d
            .as_ref()
            .expect("the board is in 3D")
    };
}

fn angles_of(preset: ViewPreset, side: ViewSide) -> (f32, f32) {
    let mut camera = OrbitCamera::default();
    camera.set_preset_on(preset, side);
    (camera.yaw_deg, camera.pitch_deg)
}

fn angles(b: &BoardBench) -> (f32, f32) {
    let camera = session!(b).camera;
    (camera.yaw_deg, camera.pitch_deg)
}

fn orbit(b: &mut BoardBench) {
    send(
        b,
        Board3dMsg::Pointer {
            viewport: [800.0, 600.0],
            gesture: Board3dGesture::Orbited { dx: 40.0, dy: 25.0 },
        },
    );
}

#[test]
fn each_viewpoint_looks_from_its_side_and_is_the_tinted_one() {
    let mut b = in_3d();
    for preset in [
        ViewPreset::Top,
        ViewPreset::Iso,
        ViewPreset::Front,
        ViewPreset::Left,
        ViewPreset::Right,
        ViewPreset::Back,
    ] {
        send(&mut b, Board3dMsg::PresetPicked(preset));
        assert_eq!(angles(&b), angles_of(preset, ViewSide::Top), "{preset:?}");
        assert_eq!(session!(b).active_preset, Some(preset));
    }
}

#[test]
fn turning_by_hand_clears_the_tinted_viewpoint() {
    let mut b = in_3d();
    send(&mut b, Board3dMsg::PresetPicked(ViewPreset::Iso));
    orbit(&mut b);
    assert_eq!(session!(b).active_preset, None);
}

#[test]
fn reset_returns_to_iso_and_keeps_flip_explode_and_lighting() {
    let mut b = in_3d();
    send(&mut b, Board3dMsg::FlipRequested);
    send(&mut b, Board3dMsg::ExplodeAmountChanged(40));
    send(&mut b, Board3dMsg::LightingCycled);
    orbit(&mut b);
    send(&mut b, Board3dMsg::ResetRequested);
    assert_eq!(angles(&b), angles_of(ViewPreset::Iso, ViewSide::Bottom));
    let s = session!(b);
    assert_eq!(s.active_preset, Some(ViewPreset::Iso));
    assert_eq!(s.side, ViewSide::Bottom);
    assert!((s.explode.amount - 0.4).abs() < 1e-6);
    assert_eq!(s.lighting, LightingPreset::default().next());
}

#[test]
fn flip_turns_the_board_over_in_frames_and_a_second_flip_comes_back() {
    let mut b = in_3d();
    let before = session!(b).camera;
    send(&mut b, Board3dMsg::FlipRequested);
    assert_eq!(session!(b).side, ViewSide::Bottom);
    assert!(session!(b).is_flipping());

    let start = Instant::now();
    send(&mut b, Board3dMsg::AnimationTicked(start));
    send(
        &mut b,
        Board3dMsg::AnimationTicked(start + Duration::from_millis(FLIP_DURATION_MS + 1)),
    );
    assert!(!session!(b).is_flipping());
    assert_eq!(session!(b).camera, before.flipped());

    send(&mut b, Board3dMsg::FlipRequested);
    assert_eq!(session!(b).side, ViewSide::Top);
}

#[test]
fn a_viewpoint_applies_on_the_side_shown() {
    let mut b = in_3d();
    send(&mut b, Board3dMsg::FlipRequested);
    send(&mut b, Board3dMsg::PresetPicked(ViewPreset::Front));
    assert_eq!(angles(&b), angles_of(ViewPreset::Front, ViewSide::Bottom));
}

#[test]
fn the_explode_slider_and_its_two_boxes_set_how_the_board_separates() {
    let mut b = in_3d();
    send(&mut b, Board3dMsg::ExplodePopoverToggled);
    assert!(session!(b).explode_open);
    send(&mut b, Board3dMsg::ExplodeAmountChanged(70));
    assert!((session!(b).explode.amount - 0.7).abs() < 1e-6);

    send(&mut b, Board3dMsg::ExplodeLayersToggled(false));
    assert!(!session!(b).explode.separate_copper_and_mask);
    send(&mut b, Board3dMsg::ExplodePartsToggled(false));
    assert!(!session!(b).explode.lift_components);
    send(&mut b, Board3dMsg::ExplodeLayersToggled(true));
    send(&mut b, Board3dMsg::ExplodePartsToggled(true));
    assert!(session!(b).explode.separate_copper_and_mask);
    assert!(session!(b).explode.lift_components);

    send(&mut b, Board3dMsg::ExplodeAmountChanged(0));
    assert!(session!(b).explode.is_assembled(), "0 % is assembled");
}

#[test]
fn an_explode_past_100_percent_stops_at_100() {
    let mut b = in_3d();
    send(&mut b, Board3dMsg::ExplodeAmountChanged(200));
    assert!((session!(b).explode.amount - 1.0).abs() < 1e-6);
}

#[test]
fn lighting_cycles_through_four_presets_with_a_toast() {
    let mut b = in_3d();
    let start = session!(b).lighting;
    let mut seen = vec![start];
    for _ in 0..LightingPreset::ALL.len() {
        send(&mut b, Board3dMsg::LightingCycled);
        seen.push(session!(b).lighting);
    }
    assert_eq!(LightingPreset::ALL.len(), 4);
    assert_eq!(seen.first(), seen.last(), "four steps come back");
    let toast = session!(b).lighting_toast.expect("the toast shows");
    send(&mut b, Board3dMsg::LightingToastExpired(toast));
    assert_eq!(session!(b).lighting_toast, None);
}

#[test]
fn esc_closes_an_open_popover_and_stays_in_3d() {
    let mut b = in_3d();
    let esc = || Message::Input(InputMsg::EscapePressed { window: None });
    send(&mut b, Board3dMsg::ExplodePopoverToggled);
    settle(&mut b.app, esc());
    assert!(!session!(b).explode_open);
    send(&mut b, Board3dMsg::ScreenshotMenuToggled);
    settle(&mut b.app, esc());
    assert!(!session!(b).shot_menu_open);
    assert!(b.app.interaction_state.board_3d.is_some());
}

#[test]
fn opening_one_popover_closes_the_other() {
    let mut b = in_3d();
    send(&mut b, Board3dMsg::ExplodePopoverToggled);
    send(&mut b, Board3dMsg::ScreenshotMenuToggled);
    assert!(!session!(b).explode_open);
    assert!(session!(b).shot_menu_open);
}

#[test]
fn the_screenshot_button_hides_the_chrome_for_a_capture() {
    let mut b = in_3d();
    send(&mut b, Board3dMsg::ScreenshotMenuToggled);
    send(&mut b, Board3dMsg::ScreenshotCopyRequested);
    assert!(session!(b).is_capturing());
    assert!(!session!(b).shot_menu_open, "the menu closes");
    assert!(session!(b).needs_frames());
}

#[test]
fn a_cancelled_save_png_captures_nothing() {
    let mut b = in_3d();
    send(&mut b, Board3dMsg::ScreenshotSavePicked(None));
    assert!(!session!(b).is_capturing());
}

#[test]
fn the_2d_switch_leaves_the_3d_view() {
    let mut b = in_3d();
    send(&mut b, Board3dMsg::Toggled);
    assert!(b.app.interaction_state.board_3d.is_none());
    assert!(!b.dirty());
}

#[test]
fn no_toolbar_control_edits_the_board_or_marks_it_dirty() {
    let mut b = in_3d();
    let before = b.json();
    for event in [
        Board3dMsg::PresetPicked(ViewPreset::Back),
        Board3dMsg::ResetRequested,
        Board3dMsg::FitRequested,
        Board3dMsg::FlipRequested,
        Board3dMsg::AnimationTicked(Instant::now()),
        Board3dMsg::ExplodePopoverToggled,
        Board3dMsg::ExplodeAmountChanged(70),
        Board3dMsg::ExplodeLayersToggled(false),
        Board3dMsg::ExplodePartsToggled(false),
        Board3dMsg::LightingCycled,
        Board3dMsg::ScreenshotMenuToggled,
    ] {
        send(&mut b, event);
    }
    assert_eq!(b.json(), before);
    assert!(!b.dirty());
    let engine = b
        .app
        .document_state
        .tabs
        .iter()
        .find(|tab| tab.path == b.path)
        .and_then(|tab| tab.cached_document.as_ref())
        .and_then(TabDocument::as_pcb_engine)
        .expect("the board's engine");
    assert!(!engine.can_undo(), "no undo step");
}
