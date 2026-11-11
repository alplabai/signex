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

fn dump(program: &SymbolSheetPreview, state: &PreviewState) -> String {
    let renderer = crate::layout_probe::headless_renderer();
    canvas::Program::<()>::draw(
        program,
        state,
        &renderer,
        &Theme::Dark,
        BOX,
        mouse::Cursor::Unavailable,
    )
    .iter()
    .map(|layer| format!("{layer:?}"))
    .collect()
}

/// A colour no theme paints with, so its paint calls can be found in a dump
/// (the renderer lists its channels as blue, green, red).
fn marker(r: u8, g: u8, b: u8) -> (signex_types::theme::Color, String) {
    let color = signex_types::theme::Color { r, g, b, a: 255 };
    let channel = |v: u8| format!("FiniteF32({:?})", f32::from(v) / 255.0);
    let text = format!(
        "r: NormalizedF32({}), g: NormalizedF32({}), b: NormalizedF32({})",
        channel(b),
        channel(g),
        channel(r)
    );
    (color, text)
}

/// The dump split into its path primitives, in paint order.
fn primitives(dump: &str) -> Vec<&str> {
    dump.split("Fill { path")
        .flat_map(|part| part.split("Stroke { path"))
        .collect()
}

/// Paper red, grid green, symbol body blue.
fn marked_program(grid: SheetGrid) -> (SymbolSheetPreview, [String; 3]) {
    let (paper, paper_text) = marker(255, 0, 0);
    let (grid_color, grid_text) = marker(0, 255, 0);
    let (body, body_text) = marker(0, 0, 255);
    let mut p = program(body_symbol(), grid);
    p.colors.paper = paper;
    p.colors.grid = grid_color;
    p.colors.body = body;
    (p, [paper_text, grid_text, body_text])
}

#[test]
fn the_preview_paints_paper_then_grid_then_the_symbol() {
    let (p, [paper, grid, body]) = marked_program(DOTS);
    let dump = dump(&p, &PreviewState::default());
    let items = primitives(&dump);
    let first = |text: &str, skip_border: bool| {
        items
            .iter()
            .position(|item| item.contains(text) && !(skip_border && item.contains("Stroke")))
    };
    let paper_at = first(&paper, false).expect("the paper is painted");
    let grid_at = first(&grid, true).expect("the grid is painted");
    let symbol_at = first(&body, false).expect("the symbol is painted");
    assert!(
        paper_at < grid_at && grid_at < symbol_at,
        "paper {paper_at}, grid {grid_at}, symbol {symbol_at}"
    );
}

/// Grid-coloured fills (dots) and strokes (lines, the paper border) of a draw.
fn grid_marks(p: &SymbolSheetPreview, grid_text: &str) -> (usize, usize) {
    let dump = dump(p, &PreviewState::default());
    primitives(&dump)
        .into_iter()
        .filter(|item| item.contains(grid_text))
        .fold((0, 0), |(fills, strokes), item| {
            if item.contains("stroke: Stroke") {
                (fills, strokes + 1)
            } else {
                (fills + 1, strokes)
            }
        })
}

#[test]
fn the_grid_follows_the_preferences() {
    let (fine, [_, grid_text, _]) = marked_program(DOTS);
    let (coarse, _) = marked_program(SheetGrid {
        pitch_mm: 5.08,
        ..DOTS
    });
    let (lines, _) = marked_program(SheetGrid {
        style: GridStyle::Lines,
        ..DOTS
    });
    let (fine_dots, _) = grid_marks(&fine, &grid_text);
    let (coarse_dots, _) = grid_marks(&coarse, &grid_text);
    assert!(
        fine_dots > coarse_dots,
        "pitch: {fine_dots} vs {coarse_dots}"
    );
    assert!(coarse_dots > 0, "the coarse grid still shows");

    let (line_fills, line_strokes) = grid_marks(&lines, &grid_text);
    assert_eq!(line_fills, 0, "Lines draws no dots");
    assert!(line_strokes > 1, "Lines draws strokes beyond the border");
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
    let (on, [_, grid_text, _]) = marked_program(DOTS);
    let (off, _) = marked_program(SheetGrid {
        visible: false,
        ..DOTS
    });
    assert!(grid_marks(&on, &grid_text).0 > 0, "grid shown when on");
    assert_eq!(grid_marks(&off, &grid_text).0, 0, "grid hidden when off");
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
