//! #1771 — the status-bar zoom reads the camera of the window it is drawn
//! in, so a fit shows from the first frame and an undocked window never
//! reports another window's zoom.

use signex_app::app::Signex;
use signex_app::canvas::CanvasSlot;
use signex_app::pcb_canvas::PcbCanvas;

fn viewport() -> iced::Rectangle {
    iced::Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0))
}

/// A zoom change with no cursor event in between must show in the readout.
#[test]
fn the_readout_follows_the_camera_without_any_cursor_event() {
    // Arrange
    let (app, _task) = Signex::new();
    let window = iced::window::Id::unique();
    let before = app.status_zoom_percent(window);
    app.interaction_state.canvas.set_viewport(viewport());

    // Act
    let changed = app.interaction_state.canvas.zoom_about_centre(true, 1.5);

    // Assert
    assert!(changed);
    let after = app.status_zoom_percent(window);
    assert!(after > before + 1.0, "{before} -> {after}");
    assert!((after - app.interaction_state.canvas.zoom_percent()).abs() < 1e-9);
}

/// An undocked window reads its own schematic and PCB cameras, and the main
/// window's readout is not disturbed by them.
#[test]
fn each_window_reads_its_own_camera() {
    // Arrange
    let (mut app, _task) = Signex::new();
    let main = iced::window::Id::unique();
    let sch_window = iced::window::Id::unique();
    let pcb_window = iced::window::Id::unique();
    let sch = CanvasSlot::new();
    sch.set_viewport(viewport());
    sch.zoom_about_centre(true, 1.5);
    let pcb = PcbCanvas::new();
    pcb.set_viewport(viewport());
    pcb.zoom_about_centre(false, 1.5);
    app.interaction_state.canvases.insert(sch_window, sch);
    app.interaction_state.pcb_canvases.insert(pcb_window, pcb);
    let interaction = &app.interaction_state;

    // Act
    let main_sch = interaction.zoom_percent_for_window(main, false);
    let main_pcb = interaction.zoom_percent_for_window(main, true);
    let own_sch = interaction.zoom_percent_for_window(sch_window, false);
    let own_pcb = interaction.zoom_percent_for_window(pcb_window, true);

    // Assert
    assert!((main_sch - interaction.canvas.zoom_percent()).abs() < 1e-9);
    assert!((main_pcb - interaction.pcb_canvas.zoom_percent()).abs() < 1e-9);
    assert!(own_sch > main_sch + 1.0, "{own_sch} vs {main_sch}");
    assert!(own_pcb < main_pcb - 1.0, "{own_pcb} vs {main_pcb}");
}
