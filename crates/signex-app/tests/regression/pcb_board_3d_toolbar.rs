//! Mockup m175 — the board 3D view's toolbar, driven through the messages it
//! sends: Left and Right, the active viewpoint, Reset, Flip and its
//! animation, Explode, Esc on an open popover, the lighting presets and
//! their toast, screenshots to the clipboard and to a PNG (and why a copy
//! failed), the model-unread toast, and where the view sits in the window. Every action is view state: no undo step, no
//! dirty mark.

use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::Rectangle;
use iced::futures::StreamExt as _;
use iced_runtime::Action;
use iced_runtime::core::clipboard::Content;
use pcb_geom::ItemRef;
use signex_app::app::{Board3dMsg, Board3dShot, Message, PcbMsg};
use signex_app::io_ports::{HeadlessAnswers, IoPorts};
use signex_app::pcb_canvas::layer_style::board_3d_colors;
use signex_gfx::camera3d::{FLIP_DURATION_MS, LightingPreset, OrbitCamera, ViewPreset, ViewSide};
use signex_renderer::board3d::BoardScene3d;
use signex_renderer::pcb::PcbLayerView;

use crate::pcb_board_3d::{BOUNDS, Fixture, PART_TOP_MM, U1, open_board, sized};

fn msg(event: Board3dMsg) -> Message {
    Message::Pcb(PcbMsg::Board3d(event))
}

/// The board in 3D, sized to [`BOUNDS`].
fn in_3d() -> Fixture {
    let mut f = open_board();
    f.menu("view_board_3d");
    sized(&mut f);
    f
}

/// The board's 3D view session (its type is not exported, so a macro).
macro_rules! session {
    ($f:expr) => {
        $f.app
            .interaction_state
            .board_3d
            .as_ref()
            .expect("the board is in 3D")
    };
}

/// The first effect `task` asks the runtime for, without running the rest.
fn first_action(task: iced::Task<Message>) -> Option<Action<Message>> {
    let mut stream = iced_runtime::task::into_stream(task)?;
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    runtime.block_on(stream.next())
}

/// A 2 x 2 opaque red shot, as the window capture hands it back cropped.
fn red_shot() -> Arc<Board3dShot> {
    Arc::new(Board3dShot {
        rgba: [255, 0, 0, 255].repeat(4),
        size: [2, 2],
    })
}

/// The camera `preset` gives on `side`, before framing.
fn angles_of(preset: ViewPreset, side: ViewSide) -> (f32, f32) {
    let mut camera = OrbitCamera::default();
    camera.set_preset_on(preset, side);
    (camera.yaw_deg, camera.pitch_deg)
}

fn drag(f: &mut Fixture) {
    use iced::{Point, mouse};
    f.pointer(
        mouse::Event::ButtonPressed(mouse::Button::Left),
        Point::new(400.0, 300.0),
    );
    f.pointer(
        mouse::Event::CursorMoved {
            position: Point::new(460.0, 260.0),
        },
        Point::new(460.0, 260.0),
    );
    f.pointer(
        mouse::Event::ButtonReleased(mouse::Button::Left),
        Point::new(460.0, 260.0),
    );
}

/// Right and Left are separate buttons; each looks from its side of the
/// side shown.
#[test]
fn left_and_right_look_from_their_sides() {
    let mut f = in_3d();
    f.view_3d(Board3dMsg::PresetPicked(ViewPreset::Right));
    let right = f.camera();
    assert_eq!(
        (right.yaw_deg, right.pitch_deg),
        angles_of(ViewPreset::Right, ViewSide::Top)
    );
    f.view_3d(Board3dMsg::PresetPicked(ViewPreset::Left));
    let left = f.camera();
    assert_eq!(
        (left.yaw_deg, left.pitch_deg),
        angles_of(ViewPreset::Left, ViewSide::Top)
    );
    assert_ne!(left.yaw_deg, right.yaw_deg);
    assert_eq!(session!(f).active_preset, Some(ViewPreset::Left));
}

/// The view opens on Top; a preset is the active one until a turn by hand.
#[test]
fn turning_by_hand_clears_the_active_preset() {
    let mut f = in_3d();
    assert_eq!(session!(f).active_preset, Some(ViewPreset::Top));
    f.view_3d(Board3dMsg::PresetPicked(ViewPreset::Iso));
    assert_eq!(session!(f).active_preset, Some(ViewPreset::Iso));
    drag(&mut f);
    assert_eq!(session!(f).active_preset, None, "a turn clears it");
    f.view_3d(Board3dMsg::PresetPicked(ViewPreset::Back));
    assert_eq!(session!(f).active_preset, Some(ViewPreset::Back));
}

/// Flip turns the board over in frames: the side changes at once, the
/// camera ends at its flipped self once the flip's time has passed, and a
/// second Flip comes back to the top.
#[test]
fn flip_turns_the_board_over_and_ends_at_the_flipped_camera() {
    let mut f = in_3d();
    let before = f.camera();
    f.view_3d(Board3dMsg::FlipRequested);
    assert_eq!(session!(f).side, ViewSide::Bottom);
    assert!(session!(f).needs_frames(), "the flip asks for frames");
    assert_eq!(f.camera(), before, "no frame yet");

    let start = Instant::now();
    f.view_3d(Board3dMsg::AnimationTicked(start));
    f.view_3d(Board3dMsg::AnimationTicked(
        start + Duration::from_millis(FLIP_DURATION_MS / 2),
    ));
    assert!(session!(f).is_flipping(), "half way");
    f.view_3d(Board3dMsg::AnimationTicked(
        start + Duration::from_millis(FLIP_DURATION_MS + 1),
    ));
    assert!(!session!(f).is_flipping());
    assert!(!session!(f).needs_frames(), "no frames once it ends");
    assert_eq!(f.camera(), before.flipped());

    f.view_3d(Board3dMsg::FlipRequested);
    assert_eq!(session!(f).side, ViewSide::Top);
    assert!(!f.can_undo());
}

/// A gesture during the flip ends it at its target first.
#[test]
fn a_gesture_during_the_flip_ends_it_at_its_target() {
    let mut f = in_3d();
    let flipped = f.camera().flipped();
    f.view_3d(Board3dMsg::FlipRequested);
    f.view_3d(Board3dMsg::AnimationTicked(Instant::now()));
    f.view_3d(Board3dMsg::Pointer {
        viewport: [BOUNDS.width, BOUNDS.height],
        gesture: signex_app::app::Board3dGesture::Zoomed(0.0),
    });
    assert!(!session!(f).is_flipping());
    assert_eq!(f.camera(), flipped);
}

/// Reset looks from Iso on the side shown, framed; flip, explode and
/// lighting stay.
#[test]
fn reset_is_iso_framed_on_the_side_shown() {
    let mut f = in_3d();
    f.view_3d(Board3dMsg::FlipRequested);
    f.view_3d(Board3dMsg::ExplodeAmountChanged(40));
    let _toast = f.app.update(msg(Board3dMsg::LightingCycled));
    drag(&mut f);
    f.view_3d(Board3dMsg::ResetRequested);
    let camera = f.camera();
    assert_eq!(
        (camera.yaw_deg, camera.pitch_deg),
        angles_of(ViewPreset::Iso, ViewSide::Bottom)
    );
    let s = session!(f);
    assert_eq!(s.active_preset, Some(ViewPreset::Iso));
    assert_eq!(s.side, ViewSide::Bottom);
    assert!((s.explode.amount - 0.4).abs() < 1e-6);
    assert_eq!(s.lighting, LightingPreset::default().next());
    for (x, y) in [(0.0, 0.0), (40.0, 30.0)] {
        let at = f.screen(x, y, 0.0);
        assert!(BOUNDS.contains(at), "corner ({x}, {y}) at {at:?}");
    }
}

/// The highest point of `footprint`'s part in `scene`, mm.
fn part_top(scene: &BoardScene3d, footprint: uuid::Uuid) -> f32 {
    let part = scene.part(footprint).expect("the part is drawn");
    scene.scene.indices[part.indices.start as usize..part.indices.end as usize]
        .iter()
        .map(|&i| scene.scene.vertices[i as usize].position[2])
        .fold(f32::MIN, f32::max)
}

/// The explode moves the scene the view draws and picks with: a lifted
/// part is still selected where it shows.
#[test]
fn explode_moves_the_drawn_scene_and_a_click_selects_a_lifted_part() {
    let mut f = in_3d();
    let board = f.live();
    let view = PcbLayerView::default();
    let assembled = session!(f).scene(&board, 0, &view, board_3d_colors());

    f.view_3d(Board3dMsg::ExplodePopoverToggled);
    assert!(session!(f).explode_open);
    f.view_3d(Board3dMsg::ExplodeAmountChanged(100));
    f.view_3d(Board3dMsg::ExplodePartsToggled(true));
    let exploded = session!(f).scene(&board, 0, &view, board_3d_colors());
    assert_ne!(
        exploded.generation, assembled.generation,
        "a new scene is drawn"
    );
    let lifted = part_top(&exploded.scene, U1);
    assert!(
        lifted > part_top(&assembled.scene, U1),
        "the part stands off the board"
    );

    // From an angle the lifted part shows away from where it sat.
    f.view_3d(Board3dMsg::PresetPicked(ViewPreset::Iso));
    let at = f.screen(12.7, 12.7, lifted);
    assert!(BOUNDS.contains(at), "the lifted part is in view at {at:?}");
    f.click(at);
    assert_eq!(f.selected(), vec![ItemRef::Footprint(U1)]);

    f.view_3d(Board3dMsg::ExplodeAmountChanged(0));
    let back = session!(f).scene(&board, 0, &view, board_3d_colors());
    assert_eq!(
        part_top(&back.scene, U1),
        part_top(&assembled.scene, U1),
        "assembled again"
    );
}

/// Esc closes an open Explode popover or screenshot menu and stops there:
/// the selection behind the view stays. With nothing open, Esc goes on to
/// the board's usual reset, which clears it.
#[test]
fn esc_closes_an_open_popover_before_the_tool_reset() {
    let mut f = in_3d();
    let esc = || Message::Input(signex_app::app::InputMsg::EscapePressed { window: None });
    let part = f.screen(12.7, 12.7, PART_TOP_MM);
    f.click(part);
    assert_eq!(f.selected(), vec![ItemRef::Footprint(U1)]);

    f.view_3d(Board3dMsg::ExplodePopoverToggled);
    f.send(esc());
    assert!(!session!(f).explode_open, "Esc closes the Explode popover");
    assert_eq!(f.selected(), vec![ItemRef::Footprint(U1)], "and stops");

    f.view_3d(Board3dMsg::ScreenshotMenuToggled);
    f.send(esc());
    assert!(
        !session!(f).shot_menu_open,
        "Esc closes the screenshot menu"
    );
    assert_eq!(f.selected(), vec![ItemRef::Footprint(U1)], "and stops");
    assert!(f.in_3d());

    f.send(esc());
    assert!(f.selected().is_empty(), "with nothing open Esc goes on");
}

/// A refused clipboard write says why in Messages.
#[test]
fn a_failed_copy_names_the_clipboard_reason() {
    let _ = signex_app::diagnostics::init_logging();
    let mut f = in_3d();
    f.view_3d(Board3dMsg::ScreenshotCopied(Err(
        iced::clipboard::Error::ClipboardOccupied,
    )));
    let logged = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message.contains("could not be copied") && e.message.contains("another program")
    });
    assert!(logged, "Messages names the reason");
}

/// Lighting cycles through four presets and back; each step shows a toast
/// whose timer only hides its own generation.
#[test]
fn lighting_cycles_in_four_and_its_toast_expires_by_generation() {
    let mut f = in_3d();
    let start = session!(f).lighting;
    let mut seen = vec![start];
    for _ in 0..LightingPreset::ALL.len() {
        // The toast's timer task is not run: it would wait two seconds.
        let _timer = f.app.update(msg(Board3dMsg::LightingCycled));
        seen.push(session!(f).lighting);
    }
    assert_eq!(seen.first(), seen.last(), "four steps come back");
    let distinct: std::collections::HashSet<_> = seen[..4].iter().collect();
    assert_eq!(distinct.len(), 4);

    let shown = session!(f).lighting_toast.expect("the toast shows");
    f.view_3d(Board3dMsg::LightingToastExpired(shown - 1));
    assert_eq!(
        session!(f).lighting_toast,
        Some(shown),
        "an older timer leaves it"
    );
    f.view_3d(Board3dMsg::LightingToastExpired(shown));
    assert_eq!(session!(f).lighting_toast, None);
}

/// The screenshot button hides the chrome, captures the window on the next
/// frame, and writes the shot to the clipboard.
#[test]
fn copy_screenshot_captures_a_frame_and_writes_the_clipboard() {
    let mut f = in_3d();
    f.view_3d(Board3dMsg::ScreenshotMenuToggled);
    f.view_3d(Board3dMsg::ScreenshotCopyRequested);
    assert!(session!(f).is_capturing(), "the chrome hides");
    assert!(!session!(f).shot_menu_open, "the menu closes");
    assert!(session!(f).needs_frames());

    let capture = f
        .app
        .update(msg(Board3dMsg::AnimationTicked(Instant::now())));
    assert!(
        matches!(
            first_action(capture),
            Some(Action::Window(iced_runtime::window::Action::Screenshot(..)))
        ),
        "the next frame captures the window"
    );
    assert!(!session!(f).needs_frames(), "one capture");

    let copy = f.app.update(msg(Board3dMsg::ScreenshotTaken {
        save_to: None,
        shot: Some(red_shot()),
    }));
    assert!(!session!(f).is_capturing(), "the chrome shows again");
    match first_action(copy) {
        Some(Action::Clipboard(iced_runtime::clipboard::Action::Write {
            content: Content::Image(image),
            ..
        })) => {
            assert_eq!((image.size.width, image.size.height), (2, 2));
            assert_eq!(image.rgba.len(), 16);
        }
        other => panic!("expected a clipboard image write, got {other:?}"),
    }
    assert!(!f.can_undo());
}

/// Save PNG asks for a path through the file dialog, captures the next
/// frame and writes the PNG there.
#[test]
fn save_png_asks_for_a_path_and_writes_the_png() {
    let mut f = in_3d();
    let target = f.path.with_file_name("board-3d.png");
    let answers = HeadlessAnswers::default();
    f.app.ui_state.io = IoPorts::headless(answers.clone());
    answers.replace_all([("path".to_string(), target.display().to_string())]);

    f.view_3d(Board3dMsg::ScreenshotSaveRequested);
    assert!(session!(f).is_capturing(), "the dialog answered");
    let capture = f
        .app
        .update(msg(Board3dMsg::AnimationTicked(Instant::now())));
    assert!(matches!(
        first_action(capture),
        Some(Action::Window(iced_runtime::window::Action::Screenshot(..)))
    ));
    f.view_3d(Board3dMsg::ScreenshotTaken {
        save_to: Some(target.clone()),
        shot: Some(red_shot()),
    });
    let png = image::open(&target).expect("the PNG is written").to_rgba8();
    assert_eq!(png.dimensions(), (2, 2));
    assert_eq!(png.get_pixel(1, 1).0, [255, 0, 0, 255]);
    assert!(!session!(f).is_capturing());
}

/// A cancelled save dialog captures nothing.
#[test]
fn a_cancelled_save_captures_nothing() {
    let mut f = in_3d();
    f.view_3d(Board3dMsg::ScreenshotSavePicked(None));
    assert!(!session!(f).is_capturing());
}

/// A part whose model could not be read also shows as a warn toast; its
/// dismissal leaves the Messages line.
#[test]
fn the_unread_toast_dismisses_and_messages_keeps_its_line() {
    let _ = signex_app::diagnostics::init_logging();
    let mut f = in_3d();
    let (source, _why) = session!(f)
        .unread
        .first()
        .cloned()
        .expect("the linked part's model could not be read");
    f.view_3d(Board3dMsg::UnreadNoticeDismissed);
    assert!(session!(f).unread.is_empty(), "the toast is gone");
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(&source) && e.message.contains("could not be read"));
    assert!(logged, "Messages keeps the line");
}

/// A redraw reports where the view sits once its size is known; the
/// screenshot crop reads it.
#[test]
fn a_redraw_reports_where_the_view_sits() {
    let mut f = in_3d();
    let placed = Rectangle {
        x: 30.0,
        y: 60.0,
        ..BOUNDS
    };
    let sent = f.redraw(placed);
    assert!(
        matches!(
            sent,
            Some(Message::Pcb(PcbMsg::Board3d(Board3dMsg::ViewportPlaced(at))))
                if at == [30.0, 60.0]
        ),
        "{sent:?}"
    );
    assert_eq!(session!(f).origin, [30.0, 60.0]);
    assert!(f.redraw(placed).is_none(), "reported once");
}

/// No toolbar action is a board edit: no undo step, no dirty mark, the
/// board unchanged.
#[test]
fn no_toolbar_action_makes_an_undo_step_or_a_dirty_mark() {
    let mut f = in_3d();
    let before = serde_json::to_string(&f.live()).unwrap();
    for event in [
        Board3dMsg::PresetPicked(ViewPreset::Back),
        Board3dMsg::PresetPicked(ViewPreset::Front),
        Board3dMsg::ResetRequested,
        Board3dMsg::FitRequested,
        Board3dMsg::FlipRequested,
        Board3dMsg::AnimationTicked(Instant::now()),
        Board3dMsg::ExplodePopoverToggled,
        Board3dMsg::ExplodeAmountChanged(70),
        Board3dMsg::ExplodeLayersToggled(false),
        Board3dMsg::ExplodePartsToggled(false),
        Board3dMsg::ScreenshotMenuToggled,
        Board3dMsg::ViewportPlaced([12.0, 34.0]),
        Board3dMsg::UnreadNoticeDismissed,
    ] {
        f.view_3d(event);
    }
    let _timer = f.app.update(msg(Board3dMsg::LightingCycled));
    assert!(f.in_3d());
    assert!(!f.can_undo(), "no undo step");
    assert!(!f.app.document_state.is_document_dirty(&f.path));
    assert_eq!(serde_json::to_string(&f.live()).unwrap(), before);
}
