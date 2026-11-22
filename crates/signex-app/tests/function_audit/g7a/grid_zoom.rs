//! m63 Grid picker and m64 Zoom pill.

use super::*;

fn footprint_app() -> (Signex, Fixture) {
    let fx = fixture::fixture().expect("fixture");
    let mut app = fixture::fresh(ThemeId::DEFAULT);
    fixture::mount_stock(&mut app, &fx).expect("stock mounts");
    drive(
        &mut app,
        Message::File(FileMsg::Opened(Some(fx.footprint.clone()))),
    );
    assert_eq!(app.document_state.footprint_editors.len(), 1);
    app.interaction_state.last_mouse_pos = (360.0, 420.0);
    (app, fx)
}

fn footprint_step(app: &Signex) -> f64 {
    let editor = app
        .document_state
        .footprint_editors
        .values()
        .next()
        .expect("a footprint editor");
    editor.state.snap_options.grid_step_mm
}

fn active_row_step(app: &Signex) -> Option<f64> {
    let editor = app.document_state.footprint_editors.values().next()?;
    editor
        .state
        .grids
        .get(editor.state.active_grid_idx)
        .map(|row| row.step_mm)
}

fn open_picker(app: &mut Signex) {
    let _ = app.update(Message::Ui(UiMsg::GridPickerOpen));
    assert!(app.interaction_state.grid_picker.is_some());
}

fn zoomable_sample() -> (Signex, Fixture) {
    let (app, fx) = sample();
    app.interaction_state
        .active_canvas()
        .set_viewport(Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0)));
    (app, fx)
}

fn zoom(app: &Signex) -> f64 {
    app.interaction_state.active_canvas().zoom_percent()
}

#[test]
fn m63_picking_a_step_sets_the_grid_and_closes_the_picker() {
    let (mut app, _fx) = footprint_app();
    open_picker(&mut app);
    let _ = app.update(Message::Ui(UiMsg::GridPickerSelect(0.635)));
    assert_eq!(footprint_step(&app), 0.635);
    assert_eq!(active_row_step(&app), Some(0.635), "the active grid row too");
    assert!(app.interaction_state.grid_picker.is_none());
}

#[test]
fn m63_closing_the_picker_keeps_the_step() {
    let (mut app, _fx) = footprint_app();
    let before = footprint_step(&app);
    open_picker(&mut app);
    let _ = app.update(Message::Ui(UiMsg::GridPickerClose));
    assert!(app.interaction_state.grid_picker.is_none());
    assert_eq!(footprint_step(&app), before);
}

#[test]
fn m63_a_step_that_is_not_a_positive_number_is_refused() {
    let (mut app, _fx) = footprint_app();
    let before = footprint_step(&app);
    for bad in [0.0, -1.27, f64::NAN, f64::INFINITY] {
        open_picker(&mut app);
        let _ = app.update(Message::Ui(UiMsg::GridPickerSelect(bad)));
        assert_eq!(footprint_step(&app), before, "{bad} is no grid step");
        assert!(app.interaction_state.grid_picker.is_none());
    }
}

#[test]
fn m63_the_grid_properties_row_opens_the_dialog_for_that_footprint() {
    let (mut app, _fx) = footprint_app();
    open_picker(&mut app);
    let _ = app.update(Message::GridProperties(GridPropertiesMsg::OpenFromPicker));
    assert!(app.ui_state.grid_properties.is_some());
    assert!(app.interaction_state.grid_picker.is_none());
}

#[test]
fn m64_a_preset_sets_that_zoom_closes_the_menu_and_is_no_edit() {
    let (mut app, _fx) = zoomable_sample();
    for percent in [200_u16, 50, 400, 100] {
        let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
        assert!(app.interaction_state.zoom_presets_open);
        let _ = app.update(Message::Ui(UiMsg::ZoomPresetPicked(percent)));
        assert!(
            (zoom(&app) - f64::from(percent)).abs() < 0.01,
            "{percent}%: the canvas reads {}",
            zoom(&app)
        );
        assert!(!app.interaction_state.zoom_presets_open);
    }
    assert!(is_clean(&app));
}

#[test]
fn m64_dismissing_the_presets_keeps_the_zoom() {
    let (mut app, _fx) = zoomable_sample();
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetPicked(200)));
    let before = zoom(&app);
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetsDismissed));
    assert!(!app.interaction_state.zoom_presets_open);
    assert_eq!(zoom(&app), before);
}

#[test]
fn m64_the_percentage_toggles_the_preset_menu() {
    let (mut app, _fx) = zoomable_sample();
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
    assert!(app.interaction_state.zoom_presets_open);
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
    assert!(!app.interaction_state.zoom_presets_open);
}

#[test]
fn m64_plus_zooms_in_and_minus_zooms_out() {
    let (mut app, _fx) = zoomable_sample();
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetPicked(100)));
    let start = zoom(&app);
    let _ = app.update(Message::Menu(MenuMessage::ZoomIn));
    let zoomed_in = zoom(&app);
    assert!(zoomed_in > start, "{start} -> {zoomed_in}");
    let _ = app.update(Message::Menu(MenuMessage::ZoomOut));
    assert!(zoom(&app) < zoomed_in);
    assert!(is_clean(&app));
}

#[test]
fn m64_fit_all_closes_the_menu_and_edits_nothing() {
    let (mut app, _fx) = zoomable_sample();
    let before = doc_counts(&app);
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
    drive(
        &mut app,
        Message::Ui(UiMsg::ZoomPresetCommandPicked(command("zoom_to_fit"))),
    );
    assert!(!app.interaction_state.zoom_presets_open);
    assert_eq!(doc_counts(&app), before);
    assert!(is_clean(&app));
}

#[test]
fn m64_fit_selection_with_nothing_selected_keeps_the_view() {
    let (mut app, _fx) = zoomable_sample();
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetPicked(200)));
    app.interaction_state.active_canvas_mut().selected.clear();
    let before = zoom(&app);
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
    drive(
        &mut app,
        Message::Ui(UiMsg::ZoomPresetCommandPicked(command(
            "zoom_to_selection_area",
        ))),
    );
    assert!(!app.interaction_state.zoom_presets_open);
    assert_eq!(zoom(&app), before);
}

#[test]
fn m64_a_preset_with_no_sheet_open_only_closes_the_menu() {
    let mut app = fixture::fresh(ThemeId::DEFAULT);
    let before = zoom(&app);
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetPicked(400)));
    assert!(!app.interaction_state.zoom_presets_open);
    assert_eq!(zoom(&app), before);
}
