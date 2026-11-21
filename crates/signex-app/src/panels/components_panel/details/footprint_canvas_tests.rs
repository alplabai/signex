//! The preview paints what the editor paints and owns a zoom/pan camera
//! that is fitted on first show.

use iced::widget::canvas::Program;
use iced::{Event, Point, Rectangle, mouse};
use signex_library::primitive::footprint::ChamferedCorners;
use signex_library::primitive::{
    Drill, Footprint, FpGraphic, FpGraphicKind, Pad, PadKind, PadShape, Polygon,
};

use super::footprint_scale::{
    drawing_area, length_label, round_length_mm, scale_bar, size_label, size_label_bounds,
};
use super::{FIT_PAD, FootprintPreview, PreviewState};
use crate::layout_probe::headless_renderer;
use crate::library::editor::footprint::canvas::{FootprintCanvas, FootprintCanvasState};
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::messages::LibraryMessage;
use crate::library::state::EditorAddress;

const BOX: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 320.0,
    height: 200.0,
};

fn pad(number: &str, at: [f64; 2], shape: PadShape, kind: PadKind) -> Pad {
    Pad {
        number: number.into(),
        kind,
        shape,
        size: [1.2, 1.2],
        position: at,
        ..Pad::default()
    }
}

/// Rect SMD pad, drilled round through-hole pad, custom-outline pad, a
/// silk line and a silk text.
fn fixture() -> Footprint {
    let mut fp = Footprint::empty("FX");
    let mut tht = pad("2", [1.5, 0.0], PadShape::Round, PadKind::Tht);
    tht.drill = Some(Drill {
        diameter: 0.6,
        slot_length: None,
    });
    fp.pads = vec![
        pad("1", [-1.5, 0.0], PadShape::Rect, PadKind::Smd),
        tht,
        pad(
            "3",
            [0.0, 2.0],
            PadShape::Custom(Polygon::new(vec![[-0.5, 0.5], [0.5, 0.5], [0.0, -0.5]])),
            PadKind::Smd,
        ),
    ];
    fp.silk_f = vec![
        FpGraphic {
            kind: FpGraphicKind::Line {
                from: [-2.0, -1.0],
                to: [2.0, -1.0],
            },
            stroke_width: 0.15,
            filled: false,
            sketch_entity: None,
        },
        FpGraphic {
            kind: FpGraphicKind::Text {
                position: [-1.0, -2.0],
                content: "REF**".into(),
                size: 0.8,
                frame: None,
            },
            stroke_width: 0.0,
            filled: false,
            sketch_entity: None,
        },
    ];
    fp
}

/// What a draw produced, as the renderer's debug dump: every path
/// primitive and text run in paint order.
struct Painted {
    dump: String,
}

impl Painted {
    fn paths(&self) -> usize {
        self.dump.matches("Fill { path").count() + self.dump.matches("Stroke { path").count()
    }

    fn texts(&self) -> usize {
        self.dump.matches("content: ").count()
    }

    fn has_text(&self, content: &str) -> bool {
        self.dump.contains(&format!("content: {content:?}"))
    }
}

fn painted<G: std::fmt::Debug>(layers: Vec<G>) -> Painted {
    Painted {
        dump: layers.iter().map(|l| format!("{l:?}")).collect(),
    }
}

fn preview_of(fp: &Footprint) -> crate::library::editor::footprint::canvas::PreviewFootprint {
    crate::library::editor::footprint::canvas::PreviewFootprint::build(fp)
}

fn show(model: &crate::library::editor::footprint::canvas::PreviewFootprint) -> PreviewState {
    let mut state = PreviewState::default();
    first_event(model, &mut state);
    state
}

fn first_event(
    model: &crate::library::editor::footprint::canvas::PreviewFootprint,
    state: &mut PreviewState,
) {
    let program = FootprintPreview::new(model);
    let moved = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(10.0, 10.0),
    });
    let _ = Program::<()>::update(&program, state, &moved, BOX, mouse::Cursor::Unavailable);
}

fn hold_ctrl(program: &FootprintPreview<'_>, state: &mut PreviewState) {
    // The platform modifier: Cmd on macOS, Ctrl elsewhere.
    let ctrl = Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
        if cfg!(target_os = "macos") {
            iced::keyboard::Modifiers::LOGO
        } else {
            iced::keyboard::Modifiers::CTRL
        },
    ));
    let _ = Program::<()>::update(program, state, &ctrl, BOX, mouse::Cursor::Unavailable);
}

fn draw_preview(
    model: &crate::library::editor::footprint::canvas::PreviewFootprint,
    state: &PreviewState,
) -> Painted {
    let program = FootprintPreview::new(model);
    let renderer = headless_renderer();
    painted(Program::<()>::draw(
        &program,
        state,
        &renderer,
        &iced::Theme::Dark,
        BOX,
        mouse::Cursor::Unavailable,
    ))
}

fn draw_editor(fp: &Footprint, at: &PreviewState) -> Painted {
    let editor = FootprintEditorState::from_footprint(fp);
    let cache = iced::widget::canvas::Cache::default();
    let program = FootprintCanvas {
        state: &editor,
        address: EditorAddress::new(
            std::path::PathBuf::new(),
            String::new(),
            signex_library::RowId::from_uuid(uuid::Uuid::nil()),
        ),
        bg_color: crate::library::editor::footprint::canvas::BACKDROP_BG,
        grid_color: crate::library::editor::footprint::canvas::BACKDROP_GRID,
        cache: &cache,
        sketch: None,
        silk_f: &fp.silk_f,
        silk_b: &fp.silk_b,
        zoom_step: 1.1,
        trackpad_pan: false,
        wheel_mode: Default::default(),
    };
    let camera = FootprintCanvasState::with_view(at.scale(), at.offset());
    let renderer = headless_renderer();
    painted(Program::<LibraryMessage>::draw(
        &program,
        &camera,
        &renderer,
        &iced::Theme::Dark,
        BOX,
        mouse::Cursor::Unavailable,
    ))
}

/// The pads are drawn as the PCB canvas draws them (#2348): a top-copper pad
/// in the colour the Layers panel lists Top Copper with, a through-hole pad
/// in the Multi-Layer colour with its drill as an opening. The custom outline
/// is board copper too since #2242; a pad the board cannot hold (a chamfered
/// rectangle) is left to the editor's renderer.
#[test]
fn preview_pads_are_the_board_lowering_in_the_board_palette() {
    use crate::pcb_canvas::layer_style::{board_layer_colour, multi_layer_colour};
    use signex_gfx::style::ColorSlot;
    use signex_renderer::theme::ResolvedTheme;
    use signex_types::pcb_layer::{CopperLayer, PcbLayer};
    let rgba = |c: [f32; 4]| iced::Color {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    };
    let model = preview_of(&fixture());
    let pads = &model.board_pads().top_polygons;
    assert_eq!(pads.len(), 4, "pads 1, 2 and 3 and pad 2's drill");
    let top_copper = board_layer_colour(PcbLayer::Copper(CopperLayer::Top));
    assert_eq!(rgba(pads[0].fill_color), top_copper);
    assert_eq!(rgba(pads[1].fill_color), multi_layer_colour());
    assert_ne!(top_copper, multi_layer_colour());
    let top_count = pads
        .iter()
        .filter(|p| rgba(p.fill_color) == top_copper)
        .count();
    assert_eq!(top_count, 2, "pad 1 and the custom pad 3");

    let board = ResolvedTheme::from_canvas_colors(crate::styles::board_canvas_colors());
    let opening = board.color(ColorSlot::BoardOpening);
    let drill = pads
        .iter()
        .find(|p| p.fill_color == opening)
        .expect("pad 2's drill");
    assert!(drill.vertices.len() >= 8);
    for v in &drill.vertices {
        let r = ((v[0] - 1.5).powi(2) + v[1].powi(2)).sqrt();
        assert!((r - 0.3).abs() < 1e-4, "{v:?} on the 0.6 mm drill");
    }

    assert!(model.unheld_pads().is_empty());

    let mut chamfered = fixture();
    chamfered.pads[2].shape = PadShape::Chamfered {
        chamfer_ratio: 0.25,
        corners: ChamferedCorners::default(),
    };
    let model = preview_of(&chamfered);
    let unheld: Vec<&str> = model
        .unheld_pads()
        .iter()
        .map(|p| p.number.as_str())
        .collect();
    assert_eq!(unheld, ["3"]);
}

#[test]
fn preview_shows_drill_hole_pad_shapes_and_silk_text() {
    let fp = fixture();
    let model = preview_of(&fp);
    let with_all = draw_preview(&model, &show(&model));

    // Pad numbers 1..3, the silk string "REF**", the size readout and the
    // scale-bar label (the last two are new readouts, so 4 became 6).
    assert_eq!(
        with_all.texts(),
        6,
        "3 pad numbers + silk text + 2 readouts"
    );
    assert!(with_all.has_text("REF**"));
    assert!(with_all.has_text("2"));

    // Without the drill the through-hole pad paints one fewer fill: the
    // board draws a drill as an opening with no edge.
    let mut undrilled = fp.clone();
    undrilled.pads[1].drill = None;
    let model = preview_of(&undrilled);
    let without = draw_preview(&model, &show(&model));
    assert_eq!(with_all.paths(), without.paths() + 1);

    // A custom outline is no longer a rectangle.
    let mut rects = fp.clone();
    rects.pads[2].shape = PadShape::Rect;
    let model = preview_of(&rects);
    let as_rect = draw_preview(&model, &show(&model));
    assert_ne!(with_all.dump, as_rect.dump);

    // Silk text is drawn: dropping it removes one text run.
    let mut no_text = fp.clone();
    no_text.silk_f.pop();
    let model = preview_of(&no_text);
    assert_eq!(draw_preview(&model, &show(&model)).texts(), 5);
}

#[test]
fn first_show_fits_the_footprint_into_the_box() {
    let model = preview_of(&fixture());
    let before = PreviewState::default();
    assert_eq!(before.offset(), Point::ORIGIN);

    let state = show(&model);
    let (min_x, min_y, max_x, max_y) = model.bbox_mm().expect("pads give a bbox");
    let scale = state.scale();
    let (off, size) = (state.offset(), BOX.size());
    let corners = [(min_x, min_y), (max_x, max_y)];
    for (x, y) in corners {
        let (sx, sy) = (x as f32 * scale + off.x, y as f32 * scale + off.y);
        assert!((0.0..=size.width).contains(&sx), "x {sx} inside the box");
        assert!((0.0..=size.height).contains(&sy), "y {sy} inside the box");
    }
    // Filled to the limiting side, not left at the default scale.
    let fits_width = (max_x - min_x) as f32 * scale;
    let fits_height = (max_y - min_y) as f32 * scale;
    assert!(
        fits_width > size.width * 0.7 || fits_height > size.height * 0.7,
        "{fits_width}x{fits_height} fills the {size:?} box"
    );
}

#[test]
fn a_later_resize_does_not_refit_over_the_users_view() {
    let model = preview_of(&fixture());
    let mut state = show(&model);
    let program = FootprintPreview::new(&model);
    let over = mouse::Cursor::Available(Point::new(100.0, 100.0));
    let wheel = Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
    });
    hold_ctrl(&program, &mut state);
    let _ = Program::<()>::update(&program, &mut state, &wheel, BOX, over);
    let zoomed = state.scale();
    let bigger = Rectangle {
        width: 600.0,
        ..BOX
    };
    let moved = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(50.0, 50.0),
    });
    let _ = Program::<()>::update(&program, &mut state, &moved, bigger, over);
    assert_eq!(state.scale(), zoomed);
}

#[test]
fn a_resize_of_an_untouched_view_refits_the_footprint_to_the_new_box() {
    let model = preview_of(&fixture());
    let mut state = show(&model);
    let program = FootprintPreview::new(&model);
    let over = mouse::Cursor::Available(Point::new(100.0, 100.0));
    let bigger = Rectangle {
        width: 600.0,
        ..BOX
    };
    let moved = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(50.0, 50.0),
    });
    let _ = Program::<()>::update(&program, &mut state, &moved, bigger, over);
    let fitted = super::fitted_camera(&model, Rectangle::with_size(bigger.size()));
    assert_eq!(state.scale(), fitted.scale);
    assert_eq!(state.offset(), fitted.offset);
}

#[test]
fn the_fit_centres_the_footprint_inside_a_narrow_and_a_wide_box() {
    let model = preview_of(&fixture());
    let (min_x, min_y, max_x, max_y) = model.bbox_mm().expect("pads give a bbox");
    for size in [iced::Size::new(120.0, 156.0), iced::Size::new(700.0, 156.0)] {
        let area = Rectangle::with_size(size);
        let camera = super::fitted_camera(&model, area);
        let (scale, off) = (camera.scale, camera.offset);
        let low = (min_x as f32 * scale + off.x, min_y as f32 * scale + off.y);
        let high = (max_x as f32 * scale + off.x, max_y as f32 * scale + off.y);
        assert!(low.0 >= 0.0 && low.1 >= 0.0, "{size:?} {low:?}");
        assert!(
            high.0 <= size.width && high.1 <= size.height,
            "{size:?} {high:?}"
        );
        // Centred across the box, and down the part above the readouts.
        let drawing = drawing_area(area, (min_x, min_y, max_x, max_y), FIT_PAD);
        assert!((low.0 + high.0 - size.width).abs() < 1e-2, "{size:?}");
        assert!((low.1 + high.1 - drawing.height).abs() < 1e-2, "{size:?}");
    }
}

/// Caner's report of 2026-11-21: the size sits under the drawing, clear of
/// the footprint and courtyard and of the scale bar, in a wide and a narrow
/// preview.
#[test]
fn the_size_label_sits_at_the_bottom_clear_of_the_drawing_and_the_bar() {
    let model = preview_of(&fixture());
    let bbox = model.bbox_mm().expect("bbox");
    let label = size_label(bbox);
    let text = crate::styles::TypeScale::DEFAULT.caption;
    for size in [
        iced::Size::new(320.0, 200.0),
        iced::Size::new(150.0, 132.0),
        iced::Size::new(120.0, 156.0),
    ] {
        let area = Rectangle::with_size(size);
        let camera = super::fitted_camera(&model, area);
        let bounds = size_label_bounds(area, &label);
        assert!(bounds.y >= size.height / 2.0, "{size:?} {bounds:?}");
        assert!(bounds.y + bounds.height <= size.height, "{size:?} {bounds:?}");
        let drawing_bottom = bbox.3 as f32 * camera.scale + camera.offset.y;
        assert!(
            drawing_bottom <= bounds.y + 1e-2,
            "{size:?}: drawing ends at {drawing_bottom}, label starts at {}",
            bounds.y
        );
        let (_, bar) = scale_bar(camera.scale, size.width).expect("bar");
        let beside_bar = bounds.x >= text + bar;
        let above_bar = bounds.y + bounds.height <= size.height - text * 3.0 + 1e-2;
        assert!(beside_bar || above_bar, "{size:?} {bounds:?} bar {bar}");
    }
}

#[test]
fn wheel_zooms_about_the_cursor_and_drag_pans() {
    let model = preview_of(&fixture());
    let mut state = show(&model);
    let program = FootprintPreview::new(&model);
    let fitted = (state.scale(), state.offset());

    let anchor = Point::new(100.0, 80.0);
    let world_before = (
        (anchor.x - state.offset().x) / state.scale(),
        (anchor.y - state.offset().y) / state.scale(),
    );
    let wheel = Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Lines { x: 0.0, y: 2.0 },
    });
    let cursor = mouse::Cursor::Available(anchor);
    hold_ctrl(&program, &mut state);
    let _ = Program::<()>::update(&program, &mut state, &wheel, BOX, cursor);
    assert!(state.scale() > fitted.0, "scrolling up zooms in");
    let world_after = (
        (anchor.x - state.offset().x) / state.scale(),
        (anchor.y - state.offset().y) / state.scale(),
    );
    assert!((world_before.0 - world_after.0).abs() < 1e-3);
    assert!((world_before.1 - world_after.1).abs() < 1e-3);

    let zoomed_offset = state.offset();
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let _ = Program::<()>::update(&program, &mut state, &press, BOX, cursor);
    // A cursor move comes with the cursor at its position, as from the runtime.
    let dragged_to = Point::new(anchor.x + 30.0, anchor.y - 12.0);
    let drag = Event::Mouse(mouse::Event::CursorMoved {
        position: dragged_to,
    });
    let _ = Program::<()>::update(
        &program,
        &mut state,
        &drag,
        BOX,
        mouse::Cursor::Available(dragged_to),
    );
    assert_eq!(state.offset().x, zoomed_offset.x + 30.0);
    assert_eq!(state.offset().y, zoomed_offset.y - 12.0);
    let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
    let _ = Program::<()>::update(&program, &mut state, &release, BOX, cursor);
    let after = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(200.0, 150.0),
    });
    let _ = Program::<()>::update(&program, &mut state, &after, BOX, cursor);
    assert_eq!(state.offset().x, zoomed_offset.x + 30.0, "released: no pan");
}

#[test]
fn another_footprint_refits_the_shared_widget_state() {
    let small = preview_of(&fixture());
    let mut state = show(&small);
    let mut other = fixture();
    other.uuid = uuid::Uuid::now_v7();
    other.pads[0].position = [-10.0, 0.0];
    let big = preview_of(&other);
    let before = state.scale();
    first_event(&big, &mut state);
    assert!(state.scale() < before, "fitted to the wider footprint");
}

#[test]
fn empty_footprint_draws_backdrop_without_panicking() {
    let model = preview_of(&Footprint::empty("E"));
    let state = show(&model);
    assert_eq!(state.offset(), Point::new(160.0, 100.0));
    let drawn = draw_preview(&model, &state);
    assert!(drawn.paths() >= 3, "background and origin drawn");
}

#[test]
fn a_saved_courtyard_is_stroked_and_inside_the_fit() {
    let mut fp = fixture();
    let plain_model = preview_of(&fp);
    let plain = draw_preview(&plain_model, &show(&plain_model));
    fp.courtyard = Polygon::new(vec![[-3.0, -3.0], [3.0, -3.0], [3.0, 3.0], [-3.0, 3.0]]);
    let model = preview_of(&fp);
    let state = show(&model);
    let drawn = draw_preview(&model, &state);
    assert_eq!(drawn.paths(), plain.paths() + 1, "one courtyard stroke");
    let (min_x, min_y, max_x, max_y) = model.bbox_mm().expect("bbox");
    assert!(min_x <= -3.0 && min_y <= -3.0 && max_x >= 3.0 && max_y >= 3.0);
}

#[test]
fn silk_outside_the_pads_is_fitted_into_the_box() {
    let mut fp = fixture();
    fp.silk_f.push(FpGraphic {
        kind: FpGraphicKind::Line {
            from: [-9.0, -6.0],
            to: [9.0, 6.0],
        },
        stroke_width: 0.15,
        filled: false,
        sketch_entity: None,
    });
    let model = preview_of(&fp);
    let state = show(&model);
    let (scale, off, size) = (state.scale(), state.offset(), BOX.size());
    for (x, y) in [(-9.0_f32, -6.0_f32), (9.0, 6.0)] {
        let (sx, sy) = (x * scale + off.x, y * scale + off.y);
        assert!((0.0..=size.width).contains(&sx), "x {sx} inside the box");
        assert!((0.0..=size.height).contains(&sy), "y {sy} inside the box");
    }
}

#[test]
fn component_preview_dialog_keeps_the_preview_model_in_step() {
    use crate::library::component_preview::test_row;
    use crate::library::state::ComponentPreviewState;
    let mut state =
        ComponentPreviewState::from_row(std::path::PathBuf::new(), "T".into(), test_row());
    assert!(state.footprint_preview().is_none());
    state.set_footprint(Some(fixture()));
    let model = state
        .footprint_preview()
        .expect("model built with footprint");
    assert_eq!(model.pads().len(), 3);
    state.set_footprint(None);
    assert!(state.footprint_preview().is_none());
}

fn over_canvas() -> mouse::Cursor {
    mouse::Cursor::Available(Point::new(100.0, 80.0))
}

#[test]
fn a_right_press_is_not_captured_and_starts_no_pan() {
    let model = preview_of(&fixture());
    let mut state = show(&model);
    let program = FootprintPreview::new(&model);
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
    let action = Program::<()>::update(&program, &mut state, &press, BOX, over_canvas());
    assert!(action.is_none());
    let before = state.offset();
    let drag = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(130.0, 80.0),
    });
    let _ = Program::<()>::update(&program, &mut state, &drag, BOX, over_canvas());
    assert_eq!(state.offset(), before);
}

#[test]
fn a_plain_wheel_is_left_to_the_panel_and_ctrl_wheel_zooms() {
    let model = preview_of(&fixture());
    let mut state = show(&model);
    let program = FootprintPreview::new(&model);
    let wheel = Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
    });
    let fitted = state.scale();
    let plain = Program::<()>::update(&program, &mut state, &wheel, BOX, over_canvas());
    assert!(plain.is_none(), "not captured, so the panel scrolls");
    assert_eq!(state.scale(), fitted);

    hold_ctrl(&program, &mut state);
    let zoomed = Program::<()>::update(&program, &mut state, &wheel, BOX, over_canvas());
    assert!(zoomed.is_some());
    assert!(state.scale() > fitted);
}

#[test]
fn cursor_leaving_ends_a_pan() {
    let model = preview_of(&fixture());
    let mut state = show(&model);
    let program = FootprintPreview::new(&model);
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let _ = Program::<()>::update(&program, &mut state, &press, BOX, over_canvas());
    let left = Event::Mouse(mouse::Event::CursorLeft);
    let _ = Program::<()>::update(&program, &mut state, &left, BOX, mouse::Cursor::Unavailable);
    let before = state.offset();
    let back = Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(150.0, 90.0),
    });
    let _ = Program::<()>::update(&program, &mut state, &back, BOX, over_canvas());
    assert_eq!(state.offset(), before, "no pan without a held button");
}

#[test]
fn fab_outline_is_drawn_and_fitted() {
    let mut fp = fixture();
    let plain_model = preview_of(&fp);
    let plain = draw_preview(&plain_model, &show(&plain_model));
    fp.fab_f.push(FpGraphic {
        kind: FpGraphicKind::Rectangle {
            from: [-8.0, -5.0],
            to: [8.0, 5.0],
        },
        stroke_width: 0.1,
        filled: false,
        sketch_entity: None,
    });
    let model = preview_of(&fp);
    let drawn = draw_preview(&model, &show(&model));
    assert_eq!(drawn.paths(), plain.paths() + 1, "one fab stroke");
    let (min_x, _, max_x, _) = model.bbox_mm().expect("bbox");
    assert!(min_x <= -8.0 && max_x >= 8.0);
}

#[test]
fn small_pads_get_numbers_in_the_preview_but_not_the_editor() {
    // ~20 mm part in the box: scale well under the editor's 25 px/mm,
    // pads still over 9 px.
    let mut fp = fixture();
    fp.pads[0].position = [-10.0, 0.0];
    fp.pads[1].position = [10.0, 0.0];
    let model = preview_of(&fp);
    let state = show(&model);
    assert!(state.scale() < 25.0 && 1.2 * state.scale() >= 9.0);
    assert!(draw_preview(&model, &state).has_text("1"));
    assert_eq!(draw_editor(&fp, &state).texts(), 1, "silk text only");
}

#[test]
fn pinch_zoom_is_cursor_anchored_floored_and_ignores_no_ops() {
    let mut cam = FootprintCanvasState::with_view(10.0, Point::new(0.0, 0.0));
    let anchor = Point::new(100.0, 50.0);
    assert!(super::pinch_zoom(&mut cam, anchor, 0.5));
    assert!((cam.scale - 15.0).abs() < 1e-4);
    // The world point under the anchor stays put.
    assert!((cam.offset.x - (100.0 - 100.0 * 1.5)).abs() < 1e-3);
    assert!((cam.offset.y - (50.0 - 50.0 * 1.5)).abs() < 1e-3);
    assert!(!super::pinch_zoom(&mut cam, anchor, 0.0));
    assert!(!super::pinch_zoom(&mut cam, anchor, f32::NAN));
    for _ in 0..40 {
        super::pinch_zoom(&mut cam, anchor, -0.5);
    }
    assert_eq!(cam.scale, super::MIN_PREVIEW_SCALE);
    assert!(!super::pinch_zoom(&mut cam, anchor, -0.5));
}

/// Inside a pane scrolled by `scroll`, iced shifts the widget cursor but not
/// the event's window position. A drag still moves the footprint exactly
/// with the pointer, on the first move too.
#[test]
fn a_drag_in_a_scrolled_pane_pans_one_to_one() {
    let model = preview_of(&fixture());
    let mut state = show(&model);
    let program = FootprintPreview::new(&model);
    // Window positions; the cursor (content space) is inside the box.
    let scroll = iced::Vector::new(0.0, 60.0);
    let cursor = |at: Point| mouse::Cursor::Available(at + scroll);
    let window_at = Point::new(100.0, 20.0);
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let pressed = Program::<()>::update(&program, &mut state, &press, BOX, cursor(window_at));
    assert!(pressed.is_some(), "the press starts a pan");
    let before = state.offset();
    for to in [Point::new(130.0, 20.0), Point::new(150.0, 35.0)] {
        let moved = Event::Mouse(mouse::Event::CursorMoved { position: to });
        let _ = Program::<()>::update(&program, &mut state, &moved, BOX, cursor(to));
        assert_eq!(state.offset(), before + (to - window_at), "to {to:?}");
    }
}

/// The previews and the footprint editor paint every layer, and the
/// courtyard, in the colour the PCB canvas gives that layer (#2348, #2375).
/// Only the drawing style differs: the previews draw a solid courtyard and
/// opaque pads as on the board, the editor a dashed courtyard and
/// see-through pads.
#[test]
fn board_previews_take_the_pcb_layer_colours() {
    use crate::library::editor::footprint::layers::{FpLayer, LayerPalette};
    use crate::pcb_canvas::layer_style::board_layer_colour;
    use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer as T};
    let cases = [
        (FpLayer::FCu, PcbLayer::Copper(CopperLayer::Top)),
        (FpLayer::BCu, PcbLayer::Copper(CopperLayer::Bottom)),
        (FpLayer::FSilks, PcbLayer::Technical(T::SilkscreenTop)),
        (FpLayer::BSilks, PcbLayer::Technical(T::SilkscreenBottom)),
        (FpLayer::FFab, PcbLayer::Technical(T::FabTop)),
        (FpLayer::BFab, PcbLayer::Technical(T::FabBottom)),
        (FpLayer::EdgeCuts, PcbLayer::Technical(T::EdgeCuts)),
    ];
    assert_eq!(cases.len(), FpLayer::ORDER.len(), "every footprint layer");
    for (layer, on_board) in cases {
        assert_eq!(layer.color(), board_layer_colour(on_board), "{layer:?}");
    }
    for palette in [LayerPalette::Board, LayerPalette::Editor] {
        assert_eq!(
            palette.courtyard_colour(),
            board_layer_colour(PcbLayer::Technical(T::CourtyardTop)),
            "{palette:?}"
        );
    }
    let board = LayerPalette::Board;
    assert!(!board.dashes_courtyard(), "the courtyard is solid");
    assert_eq!(board.pad_fill_alpha(), 1.0, "pads are opaque");
    let editor = LayerPalette::Editor;
    assert!(editor.dashes_courtyard());
    assert_eq!(editor.pad_fill_alpha(), 0.85);
}

/// `with_theme_tokens` picks the board palette in every theme.
#[test]
fn theme_tokens_pick_the_board_palette() {
    use crate::library::editor::footprint::layers::LayerPalette;
    use signex_types::theme::{ThemeId, theme_tokens};
    let model = preview_of(&fixture());
    let palette = |id| {
        FootprintPreview::new(&model)
            .with_theme_tokens(&theme_tokens(id))
            .graphite
            .map(|(_, p)| p)
    };
    // The board is dark in Graphite Light too.
    assert_eq!(palette(ThemeId::GraphiteLight), Some(LayerPalette::Board));
    assert_eq!(palette(ThemeId::GraphiteDark), Some(LayerPalette::Board));
}

fn graphic_line(from: [f64; 2], to: [f64; 2]) -> FpGraphic {
    FpGraphic {
        kind: FpGraphicKind::Line { from, to },
        stroke_width: 0.1,
        filled: false,
        sketch_entity: None,
    }
}

#[test]
fn back_silk_and_back_fab_are_drawn_and_fitted() {
    let mut fp = fixture();
    let plain_model = preview_of(&fp);
    let plain = draw_preview(&plain_model, &show(&plain_model));
    fp.silk_b.push(graphic_line([-9.0, 0.0], [9.0, 0.0]));
    fp.fab_b.push(graphic_line([0.0, -6.0], [0.0, 6.0]));
    let model = preview_of(&fp);
    let drawn = draw_preview(&model, &show(&model));
    assert_eq!(
        drawn.paths(),
        plain.paths() + 2,
        "one back silk, one back fab"
    );
    let (min_x, min_y, max_x, max_y) = model.bbox_mm().expect("bbox");
    assert!(min_x <= -9.0 && max_x >= 9.0 && min_y <= -6.0 && max_y >= 6.0);
}

#[test]
fn size_readout_and_scale_bar_are_labelled() {
    let model = preview_of(&fixture());
    let state = show(&model);
    let drawn = draw_preview(&model, &state);
    assert!(drawn.has_text(&size_label(model.bbox_mm().expect("bbox"))));
    let (mm, _) = scale_bar(state.scale(), BOX.width).expect("bar");
    assert!(drawn.has_text(&length_label(mm)));
}

#[test]
fn round_length_is_one_two_or_five_times_a_power_of_ten() {
    for (target, want) in [(1.0, 1.0), (1.4, 1.0), (1.6, 2.0), (3.4, 5.0), (7.9, 10.0)] {
        assert_eq!(round_length_mm(target), Some(want), "target {target}");
    }
    assert_eq!(round_length_mm(0.036), Some(0.05));
    assert_eq!(round_length_mm(0.0), None);
    assert_eq!(round_length_mm(-2.0), None);
    assert_eq!(round_length_mm(f64::NAN), None);
}

#[test]
fn scale_bar_spans_about_a_fifth_of_the_pane() {
    for scale in [0.5_f32, 3.0, 17.0, 120.0, 900.0] {
        let (_, px) = scale_bar(scale, 320.0).expect("bar");
        let share = px / 320.0;
        assert!(
            (0.08..=0.5).contains(&share),
            "scale {scale}: share {share}"
        );
    }
    assert!(scale_bar(0.0, 320.0).is_none());
    assert!(scale_bar(10.0, 0.0).is_none());
}

#[test]
fn size_and_length_labels_trim_trailing_zeros() {
    assert_eq!(size_label((-1.0, -0.5, 1.0, 0.5)), "2 x 1 mm");
    assert_eq!(size_label((0.0, 0.0, 3.25, 1.5)), "3.25 x 1.5 mm");
    assert_eq!(length_label(0.5), "0.5 mm");
    assert_eq!(length_label(10.0), "10 mm");
    assert_eq!(length_label(0.01), "0.01 mm");
}
