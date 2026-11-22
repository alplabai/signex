//! Symbol preview: fit, paper and grid, rebuild, zoom and pan.

use super::*;
use signex_types::schematic::{
    FillType, Graphic, LibGraphic, Point, UNSCALED_DEVICE_PX_PER_LOGICAL_PX as UNSCALED,
};
use signex_types::theme::{ThemeId, canvas_colors};

fn body_symbol() -> LibSymbol {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "L:Body" })).expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-5.0, -10.0),
            end: Point::new(5.0, 10.0),
            width: 0.0,
            fill: FillType::Background,
        },
    });
    lib
}

#[test]
fn the_preview_is_a_placed_symbol_the_canvas_can_draw() {
    let (sheet, symbol) = preview_sheet(&body_symbol());
    let size = Size::new(200.0, 100.0);
    let transform = fit_transform(symbol_bounds(&sheet, &symbol), size, UNSCALED);
    let scene = build_schematic_scene(
        &sheet,
        &transform,
        &canvas_colors(ThemeId::GraphiteLight),
        Rectangle::with_size(size),
        None,
        SceneOverrides::default(),
    );
    assert!(scene.is_some(), "a body must lower to drawn primitives");
}

#[test]
fn the_symbol_is_centred_and_fits_inside_the_box() {
    let (sheet, symbol) = preview_sheet(&body_symbol());
    let size = Size::new(200.0, 156.0);
    let extent = symbol_bounds(&sheet, &symbol);
    let transform = fit_transform(extent, size, UNSCALED);
    let low = transform.world_to_screen((extent.min_x, extent.min_y));
    let high = transform.world_to_screen((extent.max_x, extent.max_y));
    assert!(low.x >= 0.0 && low.y >= 0.0, "{low:?}");
    assert!(high.x <= size.width && high.y <= size.height, "{high:?}");
    assert!(
        (low.x + high.x - size.width).abs() < 1e-3,
        "{low:?} {high:?}"
    );
    assert!(
        (low.y + high.y - size.height).abs() < 1e-3,
        "{low:?} {high:?}"
    );
}

#[test]
fn the_fit_centres_the_symbol_inside_a_narrow_and_a_wide_box() {
    let (sheet, symbol) = preview_sheet(&body_symbol());
    let extent = symbol_bounds(&sheet, &symbol);
    for size in [Size::new(120.0, 156.0), Size::new(700.0, 156.0)] {
        let transform = fit_transform(extent, size, UNSCALED);
        let low = transform.world_to_screen((extent.min_x, extent.min_y));
        let high = transform.world_to_screen((extent.max_x, extent.max_y));
        assert!(low.x >= 0.0 && low.y >= 0.0, "{size:?} {low:?}");
        assert!(
            high.x <= size.width && high.y <= size.height,
            "{size:?} {high:?}"
        );
        assert!((low.x + high.x - size.width).abs() < 1e-3, "{size:?}");
        assert!((low.y + high.y - size.height).abs() < 1e-3, "{size:?}");
    }
}

#[test]
fn a_resized_box_refits_an_untouched_symbol() {
    let p = program(body_symbol(), DOTS);
    let mut state = shown(&p);
    let narrow = Rectangle::new(iced::Point::ORIGIN, Size::new(120.0, 156.0));
    let moved = iced::Event::Mouse(mouse::Event::CursorMoved {
        position: iced::Point::new(10.0, 10.0),
    });
    let _ =
        canvas::Program::<()>::update(&p, &mut state, &moved, narrow, mouse::Cursor::Unavailable);
    let fitted = p.fitted_camera(narrow.size());
    assert_eq!(state.camera.scale, fitted.scale);
    assert_eq!(state.camera.offset, fitted.offset);
    assert_eq!(state.fitted_size, Some(narrow.size()));
}

#[test]
fn a_small_box_keeps_a_pixel_margin_for_pin_text() {
    let (sheet, symbol) = preview_sheet(&body_symbol());
    let size = Size::new(60.0, 40.0);
    let extent = symbol_bounds(&sheet, &symbol);
    let transform = fit_transform(extent, size, UNSCALED);
    let low = transform.world_to_screen((extent.min_x, extent.min_y));
    let high = transform.world_to_screen((extent.max_x, extent.max_y));
    let room = PREVIEW_TEXT_SIZE.min_px * TEXT_MARGIN_EM;
    assert!(low.x >= room - 1e-3 && low.y >= room - 1e-3, "{low:?}");
    assert!(high.x <= size.width - room + 1e-3, "{high:?}");
    assert!(high.y <= size.height - room + 1e-3, "{high:?}");
}

#[test]
fn a_tiny_symbol_is_not_blown_up_past_the_zoom_cap() {
    let extent = Aabb::new(-1.0, -0.5, 1.0, 0.5);
    let transform = fit_transform(extent, Size::new(300.0, 156.0), UNSCALED);
    assert!(
        (transform.scale - max_scale()).abs() < 1e-3,
        "{}",
        transform.scale
    );
}

const BOX: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 320.0,
    height: 200.0,
};

const DOTS: SheetGrid = SheetGrid {
    visible: true,
    pitch_mm: 1.27,
    style: GridStyle::Dots,
};

fn program(lib: LibSymbol, grid: SheetGrid) -> SymbolSheetPreview {
    SymbolSheetPreview {
        lib,
        colors: canvas_colors(ThemeId::GraphiteLight),
        grid,
        device_px_per_logical_px: UNSCALED,
        zoom_step: Camera::zoom_step_factor(Camera::ZOOM_STEP_PERCENT_DEFAULT),
    }
}

#[test]
fn the_first_draw_and_any_change_rebuild_but_the_same_symbol_does_not() {
    let lib = body_symbol();
    let colors = canvas_colors(ThemeId::GraphiteLight);
    let mut shown = None;
    assert!(
        take_rebuild(&mut shown, &(lib.clone(), colors, DOTS)),
        "first draw"
    );
    assert!(
        !take_rebuild(&mut shown, &(lib, colors, DOTS)),
        "same symbol"
    );

    let mut other = body_symbol();
    other.id = "L:Other".into();
    assert!(
        take_rebuild(&mut shown, &(other.clone(), colors, DOTS)),
        "changed symbol"
    );
    assert!(
        !take_rebuild(&mut shown, &(other.clone(), colors, DOTS)),
        "settled again"
    );

    let dark = canvas_colors(ThemeId::GraphiteDark);
    assert_ne!(colors, dark, "the two themes must differ");
    assert!(
        take_rebuild(&mut shown, &(other.clone(), dark, DOTS)),
        "changed colours"
    );

    let lines = SheetGrid {
        style: GridStyle::Lines,
        ..DOTS
    };
    assert!(
        take_rebuild(&mut shown, &(other, dark, lines)),
        "changed grid"
    );
}

/// A draw of `program` as RGBA pixels, through the headless software
/// renderer and the canvas widget (the preview's cache and all).
fn pixels(program: SymbolSheetPreview) -> Vec<u8> {
    let canvas = canvas::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill);
    let element = container::<_, Theme>(canvas).boxed();
    crate::layout_probe::draw_pixels(element, BOX.size())
}

#[derive(Clone, Copy, PartialEq)]
enum Marker {
    Paper,
    Grid,
    Body,
}

/// Paper red, grid green, symbol body blue: pure channels no theme paints
/// with, so a pixel's dominant channel names what painted it.
fn marker(r: u8, g: u8, b: u8) -> signex_types::theme::Color {
    signex_types::theme::Color { r, g, b, a: 255 }
}

fn marker_at(pixels: &[u8], x: u32, y: u32) -> Option<Marker> {
    let at = ((y * BOX.width as u32 + x) * 4) as usize;
    let [r, g, b] = [pixels[at], pixels[at + 1], pixels[at + 2]].map(i32::from);
    match (
        r > 160 && g < 90 && b < 90,
        g > 160 && r < 90 && b < 90,
        b > 160 && r < 90 && g < 90,
    ) {
        (true, ..) => Some(Marker::Paper),
        (_, true, _) => Some(Marker::Grid),
        (.., true) => Some(Marker::Body),
        _ => None,
    }
}

fn count(pixels: &[u8], marker: Marker, area: Rectangle) -> usize {
    (area.y as u32..(area.y + area.height) as u32)
        .flat_map(|y| (area.x as u32..(area.x + area.width) as u32).map(move |x| (x, y)))
        .filter(|&(x, y)| marker_at(pixels, x, y) == Some(marker))
        .count()
}

/// The paper's inside, clear of its border and of the part in the middle of it.
const PAPER_MARGIN: Rectangle = Rectangle {
    x: 12.0,
    y: 12.0,
    width: 60.0,
    height: 40.0,
};

fn marked_program(grid: SheetGrid) -> SymbolSheetPreview {
    let mut p = program(body_symbol(), grid);
    p.colors.paper = marker(255, 0, 0);
    p.colors.grid = marker(0, 255, 0);
    p.colors.body = marker(0, 0, 255);
    p.colors.body_fill = marker(0, 0, 255);
    p
}

/// The pixels the part's body covers, inset so antialiasing at its edge is out.
fn body_area(p: &SymbolSheetPreview) -> Rectangle {
    let (sheet, symbol) = preview_sheet(&p.lib);
    let extent = symbol_bounds(&sheet, &symbol);
    let transform = fit_transform(extent, BOX.size(), UNSCALED);
    let low = transform.world_to_screen((extent.min_x, extent.min_y));
    let high = transform.world_to_screen((extent.max_x, extent.max_y));
    Rectangle::new(
        iced::Point::new(low.x + 3.0, low.y + 3.0),
        Size::new(high.x - low.x - 6.0, high.y - low.y - 6.0),
    )
}

/// The stacking order shows in what survives: the grid lies over the paper,
/// so grid pixels exist, and the part lies over the grid, so none of them
/// show inside its filled body. Were the grid drawn first the paper would
/// hide it, and were the part drawn first the grid would cross its body.
#[test]
fn the_preview_paints_paper_then_grid_then_the_symbol() {
    let p = marked_program(DOTS);
    let body = body_area(&p);
    let shot = pixels(p);
    assert!(
        count(&shot, Marker::Paper, PAPER_MARGIN) > 0,
        "the paper is painted"
    );
    assert!(
        count(&shot, Marker::Grid, PAPER_MARGIN) > 0,
        "the grid is painted over the paper"
    );
    assert!(
        count(&shot, Marker::Body, body) > 0,
        "the symbol is painted"
    );
    assert_eq!(
        count(&shot, Marker::Grid, body),
        0,
        "the symbol is painted over the grid"
    );
}

#[test]
fn the_grid_follows_the_preferences() {
    let dots = |pitch_mm| {
        let shot = pixels(marked_program(SheetGrid { pitch_mm, ..DOTS }));
        count(&shot, Marker::Grid, PAPER_MARGIN)
    };
    let (fine_dots, coarse_dots) = (dots(1.27), dots(5.08));
    assert!(
        fine_dots > coarse_dots,
        "pitch: {fine_dots} vs {coarse_dots}"
    );
    assert!(coarse_dots > 0, "the coarse grid still shows");

    let lines = pixels(marked_program(SheetGrid {
        style: GridStyle::Lines,
        ..DOTS
    }));
    let dotted = pixels(marked_program(DOTS));
    // A grid line crosses the whole sample as one run; dots never do.
    let has_line_row = |shot: &[u8]| {
        let area = PAPER_MARGIN;
        (area.y as u32..(area.y + area.height) as u32).any(|y| {
            (area.x as u32..(area.x + area.width) as u32)
                .all(|x| marker_at(shot, x, y) == Some(Marker::Grid))
        })
    };
    assert!(has_line_row(&lines), "Lines draws strokes across the paper");
    assert!(!has_line_row(&dotted), "Dots draws no stroke");
}

#[test]
fn the_part_sits_on_the_paper() {
    let (sheet, symbol) = preview_sheet(&body_symbol());
    let (width, height) = sheet.paper_mm();
    assert_eq!((width, height), (297.0, 210.0));
    let extent = symbol_bounds(&sheet, &symbol);
    assert!(extent.min_x > 0.0 && extent.min_y > 0.0, "{extent:?}");
    assert!(extent.max_x < width && extent.max_y < height, "{extent:?}");
}

fn event(event: mouse::Event, state: &mut PreviewState, p: &SymbolSheetPreview) -> bool {
    over(iced::Event::Mouse(event), state, p)
}

/// Deliver `event` the way the runtime does: a cursor move comes with the
/// cursor at its position; anything else finds it over the box.
fn over(event: iced::Event, state: &mut PreviewState, p: &SymbolSheetPreview) -> bool {
    let at = match event {
        iced::Event::Mouse(mouse::Event::CursorMoved { position }) => position,
        _ => iced::Point::new(100.0, 80.0),
    };
    canvas::Program::<()>::update(p, state, &event, BOX, mouse::Cursor::Available(at)).is_some()
}

/// Inside a pane scrolled by `scroll`, iced shifts the widget cursor by the
/// scroll offset but hands the event its window position unchanged. A drag
/// still moves the drawing exactly with the pointer, on the first move too.
#[test]
fn a_drag_in_a_scrolled_pane_pans_one_to_one() {
    let p = program(body_symbol(), DOTS);
    let mut state = shown(&p);
    // Window positions; the cursor (content space) is inside the box.
    let scroll = Vector::new(0.0, 60.0);
    let window_at = iced::Point::new(100.0, 20.0);
    let press = iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let cursor = |at: iced::Point| mouse::Cursor::Available(at + scroll);
    let pressed = canvas::Program::<()>::update(&p, &mut state, &press, BOX, cursor(window_at));
    assert!(pressed.is_some(), "the press starts a pan");
    let before = state.camera.offset;
    for (step, to) in [(130.0, 20.0), (150.0, 35.0)].into_iter().enumerate() {
        let to = iced::Point::new(to.0, to.1);
        let moved = iced::Event::Mouse(mouse::Event::CursorMoved { position: to });
        let _ = canvas::Program::<()>::update(&p, &mut state, &moved, BOX, cursor(to));
        assert_eq!(
            state.camera.offset,
            before + (to - window_at),
            "move {step} follows the pointer"
        );
    }
    // Over a scrollbar or a layer above, the obstructed cursor still drags.
    let to = iced::Point::new(160.0, 40.0);
    let moved = iced::Event::Mouse(mouse::Event::CursorMoved { position: to });
    let _ = canvas::Program::<()>::update(&p, &mut state, &moved, BOX, cursor(to).obstruct());
    assert_eq!(state.camera.offset, before + (to - window_at));
}

fn hold_ctrl(state: &mut PreviewState, p: &SymbolSheetPreview) {
    over(
        // The platform modifier: Cmd on macOS, Ctrl elsewhere.
        iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
            if cfg!(target_os = "macos") {
                iced::keyboard::Modifiers::LOGO
            } else {
                iced::keyboard::Modifiers::CTRL
            },
        )),
        state,
        p,
    );
}

#[test]
fn the_grid_toggle_hides_the_grid() {
    let on = pixels(marked_program(DOTS));
    let off = pixels(marked_program(SheetGrid {
        visible: false,
        ..DOTS
    }));
    assert!(
        count(&on, Marker::Grid, PAPER_MARGIN) > 0,
        "grid shown when on"
    );
    assert_eq!(
        count(&off, Marker::Grid, PAPER_MARGIN),
        0,
        "grid hidden when off"
    );
}

fn shown(p: &SymbolSheetPreview) -> PreviewState {
    let mut state = PreviewState::default();
    event(
        mouse::Event::CursorMoved {
            position: iced::Point::new(10.0, 10.0),
        },
        &mut state,
        p,
    );
    state
}

#[test]
fn the_first_show_fits_the_part_to_the_box() {
    let p = program(body_symbol(), DOTS);
    let state = shown(&p);
    let fitted = p.fitted_camera(BOX.size());
    assert_eq!(state.camera.scale, fitted.scale);
    assert_eq!(state.camera.offset, fitted.offset);
    assert_eq!(state.fitted_for, Some(p.fit_key()));
    assert_eq!(p.fit_key().0, "L:Body");
    assert!(state.camera.scale > 0.0);
}

#[test]
fn a_plain_wheel_is_left_to_the_panel_and_ctrl_wheel_zooms() {
    let p = program(body_symbol(), DOTS);
    let mut state = shown(&p);
    let wheel = mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
    };
    let fitted = state.camera.scale;
    assert!(!event(wheel, &mut state, &p), "not captured");
    assert_eq!(state.camera.scale, fitted);

    hold_ctrl(&mut state, &p);
    assert!(event(wheel, &mut state, &p), "Ctrl+wheel is captured");
    assert!(state.camera.scale > fitted, "{}", state.camera.scale);
}

#[test]
fn left_drag_pans_and_a_right_press_does_not() {
    let p = program(body_symbol(), DOTS);
    let mut state = shown(&p);
    let drag = mouse::Event::CursorMoved {
        position: iced::Point::new(130.0, 80.0),
    };
    let before = state.camera.offset;

    assert!(!event(
        mouse::Event::ButtonPressed(mouse::Button::Right),
        &mut state,
        &p
    ));
    event(drag, &mut state, &p);
    assert_eq!(state.camera.offset, before, "a right press starts no pan");

    assert!(event(
        mouse::Event::ButtonPressed(mouse::Button::Left),
        &mut state,
        &p
    ));
    event(drag, &mut state, &p);
    assert_eq!(state.camera.offset, before + Vector::new(30.0, 0.0));
}

#[test]
fn the_cursor_leaving_ends_a_pan() {
    let p = program(body_symbol(), DOTS);
    let mut state = shown(&p);
    event(
        mouse::Event::ButtonPressed(mouse::Button::Left),
        &mut state,
        &p,
    );
    assert!(state.pan_from.is_some());
    event(mouse::Event::CursorLeft, &mut state, &p);
    assert!(state.pan_from.is_none());
    let before = state.camera.offset;
    event(
        mouse::Event::CursorMoved {
            position: iced::Point::new(160.0, 90.0),
        },
        &mut state,
        &p,
    );
    assert_eq!(state.camera.offset, before, "the pan is over");
}

#[test]
fn an_edited_body_under_the_same_id_refits() {
    let first = program(body_symbol(), DOTS);
    let mut state = shown(&first);
    let mut taller = body_symbol();
    taller.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-5.0, -30.0),
            end: Point::new(5.0, 30.0),
            width: 0.0,
            fill: FillType::Background,
        },
    });
    let second = program(taller, DOTS);
    assert_eq!(first.lib.id, second.lib.id);
    event(
        mouse::Event::CursorMoved {
            position: iced::Point::new(10.0, 10.0),
        },
        &mut state,
        &second,
    );
    let fitted = second.fitted_camera(BOX.size());
    assert_eq!(state.camera.scale, fitted.scale);
    assert_eq!(state.fitted_for, Some(second.fit_key()));
    assert_ne!(first.fit_key(), second.fit_key());
}

/// The preview draws its text through a readability clamp the sheet no
/// longer has: past 64 px it stops growing and under 6 px it stops shrinking.
#[test]
fn the_preview_draws_text_inside_its_clamp() {
    let at = |scale: f32| {
        let transform = ScreenTransform {
            offset_x: 0.0,
            offset_y: 0.0,
            scale,
            device_px_per_logical_px: UNSCALED,
        };
        preview_draw_options(&transform).text_px_for_test(TEXT_HEIGHT_MM)
    };
    assert_eq!(at(10_000.0), 64.0, "far past the ceiling");
    assert_eq!(at(0.01), 6.0, "far under the floor");
}

#[test]
fn pinch_zooms_about_the_cursor_without_ctrl_and_ignores_zero_delta() {
    let p = program(body_symbol(), DOTS);
    let mut state = shown(&p);
    let before = state.camera.scale;
    assert!(!event(
        mouse::Event::PinchGesture { delta: 0.0 },
        &mut state,
        &p
    ));
    assert!(event(
        mouse::Event::PinchGesture { delta: 0.25 },
        &mut state,
        &p
    ));
    assert!((state.camera.scale / before - 1.25).abs() < 1e-4);
    assert!(event(
        mouse::Event::PinchGesture { delta: -0.25 },
        &mut state,
        &p
    ));
    assert!(state.camera.scale < before * 1.25);
}

#[test]
fn a_resize_after_the_user_zoomed_keeps_the_symbol_view() {
    let p = program(body_symbol(), DOTS);
    let mut state = shown(&p);
    hold_ctrl(&mut state, &p);
    assert!(event(
        mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        },
        &mut state,
        &p,
    ));
    let kept = state.camera.scale;
    let narrow = Rectangle::new(iced::Point::ORIGIN, Size::new(120.0, 156.0));
    let moved = iced::Event::Mouse(mouse::Event::CursorMoved {
        position: iced::Point::new(10.0, 10.0),
    });
    let _ =
        canvas::Program::<()>::update(&p, &mut state, &moved, narrow, mouse::Cursor::Unavailable);
    assert_eq!(state.camera.scale, kept);
}
