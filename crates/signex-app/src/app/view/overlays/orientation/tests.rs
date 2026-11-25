//! Mockup row 194: the orientation tile's arrows and caption per view
//! state, the Home button's accent dot, the View Options eye, and the tile,
//! Home and the board's zoom pill inside the canvas with a right dock open.

use std::path::PathBuf;

use iced::{Rectangle, Size};
use signex_gfx::view_rotation::ViewRotation;
use signex_types::schematic::PageOrigin;

use super::axes::{ScreenAxes, board_caption};
use super::{ClusterRects, ViewOrientation, cluster_rects};
use crate::app::{FileMsg, Message, PcbMsg, PcbViewMsg, Signex, ViewRotationMsg};
use crate::canvas::overlay_camera_tests::{graphite_app_showing, graphite_app_with_selected_wire};
use crate::dock::PanelPosition;
use crate::layout_probe::{LaidOutText, lay_out_texts};

/// The size the headless renders lay the window out at.
const LAID_OUT: Size = Size::new(1280.0, 800.0);

/// Clockwise screen degrees of `axis` from screen right, whole, in `[0, 360)`.
fn screen_angle(axis: [f32; 2]) -> f32 {
    let degrees = axis[1].atan2(axis[0]).to_degrees().round();
    (degrees + 360.0) % 360.0
}

fn board_angles(degrees: f32, flipped: bool) -> (f32, f32) {
    let axes = ScreenAxes::of_board(ViewRotation::from_degrees(degrees).with_mirror(flipped));
    (screen_angle(axes.x), screen_angle(axes.y))
}

/// `text` without the Unicode isolation marks a message argument may carry.
fn plain(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
        .collect()
}

fn i18n() -> std::sync::Arc<signex_i18n::I18n> {
    signex_i18n::I18n::en_us()
}

/// Screen angles: 0 is right, 90 down, 180 left, 270 up. At 0° +X points
/// right and +Y up; each quarter turn turns both clockwise; Flip mirrors X
/// only.
#[test]
fn the_arrows_turn_with_the_view_and_flip_mirrors_x() {
    assert_eq!(board_angles(0.0, false), (0.0, 270.0));
    assert_eq!(board_angles(90.0, false), (90.0, 0.0));
    assert_eq!(board_angles(180.0, false), (180.0, 90.0));
    assert_eq!(board_angles(270.0, false), (270.0, 180.0));
    assert_eq!(board_angles(0.0, true), (180.0, 270.0));
    // Flipped, then turned: the board canvas's own transform.
    assert_eq!(board_angles(90.0, true), (270.0, 0.0));
}

/// A sheet never turns; its +Y follows the page origin the sheet counts Y
/// from.
#[test]
fn a_sheet_points_y_by_its_page_origin() {
    let up = ScreenAxes::of_sheet(PageOrigin::LowerLeft);
    assert_eq!((screen_angle(up.x), screen_angle(up.y)), (0.0, 270.0));
    let down = ScreenAxes::of_sheet(PageOrigin::UpperLeft);
    assert_eq!((screen_angle(down.x), screen_angle(down.y)), (0.0, 90.0));
}

#[test]
fn the_caption_names_the_turn_and_the_bottom_view() {
    let i18n = i18n();
    let caption =
        |degrees, flipped| board_caption(&i18n, degrees, flipped).map(|c| plain(&c.into_string()));
    assert_eq!(caption(0.0, false), None, "no caption at home");
    assert_eq!(caption(90.0, false).as_deref(), Some("90°"));
    assert_eq!(caption(0.0, true).as_deref(), Some("Bottom"));
    assert_eq!(caption(90.0, true).as_deref(), Some("Bottom · 90°"));
    assert_eq!(caption(37.25, false).as_deref(), Some("37.3°"));
    // Reads as the status bar's angle does: 359.96° shows as 0°.
    assert_eq!(caption(359.96, false), None);
}

/// The accent dot shows exactly while the view is turned or flipped; a
/// sheet is always home and has no Flip caption.
#[test]
fn the_home_dot_shows_while_the_view_is_away_from_home() {
    let i18n = i18n();
    let board = |degrees: f32, flipped: bool| ViewOrientation::Board {
        rotation: ViewRotation::from_degrees(degrees).with_mirror(flipped),
        tile_shown: true,
    };
    assert!(!board(0.0, false).away_from_home(&i18n));
    assert!(board(90.0, false).away_from_home(&i18n));
    assert!(board(0.0, true).away_from_home(&i18n));
    for origin in [PageOrigin::LowerLeft, PageOrigin::UpperLeft] {
        let sheet = ViewOrientation::Sheet { origin };
        assert!(!sheet.away_from_home(&i18n));
        assert_eq!(sheet.caption(&i18n), None);
        assert!(sheet.tile_shown());
    }
}

fn holds(outer: Rectangle, inner: Rectangle) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

/// The tile right-aligned with the pill one medium gap above it, Home a
/// small gap left of the pill on its line, all inside `canvas`.
fn assert_cluster_inside(canvas: Rectangle, rects: ClusterRects, case: &str) {
    const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
    for (name, rect) in [
        ("pill", Some(rects.pill)),
        ("home", Some(rects.home)),
        ("tile", rects.tile),
    ] {
        if let Some(rect) = rect {
            assert!(
                holds(canvas, rect),
                "{case}: {name} {rect:?} leaves {canvas:?}"
            );
        }
    }
    let (pill, home) = (rects.pill, rects.home);
    assert_eq!(home.y, pill.y, "{case}: Home is off the pill's line");
    assert!(
        (pill.x - (home.x + home.width) - SP.s).abs() < 1e-3,
        "{case}: Home gap"
    );
    if let Some(tile) = rects.tile {
        assert!(
            (tile.x + tile.width - (pill.x + pill.width)).abs() < 1e-3,
            "{case}: tile right edge"
        );
        assert!(
            (pill.y - (tile.y + tile.height) - SP.m).abs() < 1e-3,
            "{case}: tile gap"
        );
    }
}

fn assert_right_dock_open(app: &Signex) {
    let dock = &app.document_state.dock;
    assert!(dock.has_panels(PanelPosition::Right), "no right dock");
    assert!(
        !dock.is_collapsed(PanelPosition::Right),
        "right dock collapsed"
    );
}

/// The main window showing an empty board, laid out at [`LAID_OUT`].
fn board_app() -> (Signex, iced::window::Id, PathBuf) {
    let (mut app, main) = graphite_app_with_selected_wire();
    let path = PathBuf::from("/w/orientation.snxpcb");
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "orientation".to_string(),
        result: Ok(Box::new(
            crate::app::handlers::document_files::save_async_tests::empty_board(),
        )),
    }));
    assert!(app.has_active_pcb());
    app.sync_pcb_canvas_from_visible_board();
    app.ui_state.window_size = (LAID_OUT.width, LAID_OUT.height);
    (app, main, path)
}

fn canvas_of(app: &Signex) -> Rectangle {
    app.canvas_bounds_sized(None, (LAID_OUT.width, LAID_OUT.height))
}

fn rects_of(app: &Signex, main: iced::window::Id) -> ClusterRects {
    let orientation = app.view_orientation(main).expect("a canvas in view");
    let has_caption = orientation.caption(&app.ui_state.i18n).is_some();
    cluster_rects(
        canvas_of(app),
        orientation.tile_shown().then_some(has_caption),
    )
}

/// Texts the main window lays out whose centre is inside `rect`.
fn texts_in(app: &Signex, main: iced::window::Id, rect: Rectangle) -> Vec<LaidOutText> {
    lay_out_texts(app.view(main), LAID_OUT)
        .into_iter()
        .filter(|t| rect.contains(t.bounds.center()))
        .collect()
}

/// On a turned, flipped board with a right dock open, at every dock width,
/// the tile, Home and the board's zoom pill sit inside the canvas, and the
/// layout draws the caption in the tile and Fit in the pill.
#[test]
fn the_board_cluster_stays_inside_the_canvas_with_a_right_dock() {
    let (mut app, main, _) = board_app();
    assert_right_dock_open(&app);
    let _ = app.update(Message::Pcb(PcbMsg::ViewRotation(
        ViewRotationMsg::AngleSet(90.0),
    )));
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::BoardFlipToggled)));
    for width in [crate::dock::MIN_SIDE_DOCK_WIDTH, 320.0, 480.0] {
        app.ui_state.right_width = width;
        let rects = rects_of(&app, main);
        let case = format!("right dock {width} px");
        assert!(rects.tile.is_some(), "{case}: no tile");
        assert_cluster_inside(canvas_of(&app), rects, &case);
        let tile = rects.tile.expect("tile");
        assert!(
            texts_in(&app, main, tile)
                .iter()
                .any(|t| plain(&t.text) == "Bottom · 90°"),
            "{case}: no caption in the tile"
        );
        assert!(
            texts_in(&app, main, rects.pill)
                .iter()
                .any(|t| t.text == "Fit"),
            "{case}: no board zoom pill"
        );
    }
}

/// On a sheet the same cluster sits by the sheet's zoom pill, the tile with
/// no caption row: a sheet has no Flip and does not turn.
#[test]
fn the_sheet_cluster_has_no_flip_caption_and_stays_inside_the_canvas() {
    let (mut app, main) = graphite_app_showing(|sheet| sheet.page.origin = PageOrigin::UpperLeft);
    app.ui_state.window_size = (LAID_OUT.width, LAID_OUT.height);
    assert_right_dock_open(&app);
    let orientation = app.view_orientation(main).expect("a sheet in view");
    assert_eq!(
        orientation,
        ViewOrientation::Sheet {
            origin: PageOrigin::UpperLeft
        }
    );
    assert_eq!(orientation.caption(&app.ui_state.i18n), None);
    let rects = rects_of(&app, main);
    let tile = rects.tile.expect("the tile shows on a sheet");
    assert_eq!(tile.size(), Size::from(super::tile::tile_size(false)));
    assert_cluster_inside(canvas_of(&app), rects, "sheet");
    let in_tile: Vec<String> = texts_in(&app, main, tile)
        .iter()
        .map(|t| plain(&t.text))
        .collect();
    assert!(
        !in_tile
            .iter()
            .any(|t| t.starts_with("Bottom") || t.ends_with('°')),
        "a sheet tile has a caption: {in_tile:?}"
    );
}

/// The View Options eye hides the tile and keeps Home and the pill; the
/// choice is kept in the board's view (`PcbLayerView`) and in the panel.
#[test]
fn the_view_options_eye_hides_the_tile_and_is_kept_with_the_board_view() {
    let (mut app, main, path) = board_app();
    assert!(rects_of(&app, main).tile.is_some());
    let _ = app.update(Message::Pcb(PcbMsg::View(
        PcbViewMsg::OrientationTileVisibilityToggled,
    )));
    assert!(!app.pcb_layer_view(&path).orientation_tile_visible());
    assert!(
        !app.document_state
            .panel_ctx
            .pcb_layers
            .orientation_tile_visible
    );
    let rects = rects_of(&app, main);
    assert_eq!(rects.tile, None, "the eye did not hide the tile");
    assert_cluster_inside(canvas_of(&app), rects, "tile hidden");
    let _ = app.update(Message::Pcb(PcbMsg::View(
        PcbViewMsg::OrientationTileVisibilityToggled,
    )));
    assert!(
        rects_of(&app, main).tile.is_some(),
        "the eye did not show it again"
    );
}

/// No tile in another window: the cluster is the main window's.
#[test]
fn only_the_main_window_shows_the_tile() {
    let (app, _, _) = board_app();
    assert!(app.view_orientation(iced::window::Id::unique()).is_none());
}
