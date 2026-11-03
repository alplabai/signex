//! What the View menu's rows do.

use super::*;

fn framed(app: &Signex) {
    app.interaction_state
        .active_canvas()
        .set_viewport(iced::Rectangle::new(
            iced::Point::ORIGIN,
            iced::Size::new(800.0, 600.0),
        ));
}

#[test]
fn zoom_in_and_out_step_the_camera_about_the_centre() {
    let mut app = one_symbol_selected();
    framed(&app);
    let before = probe(&app);

    let _ = click(&mut app, "View/Zoom In");
    let zoomed_in = probe(&app);
    assert!(zoomed_in.zoom_percent > before.zoom_percent, "no zoom in");
    assert_eq!(
        zoomed_in,
        Probe {
            zoom_percent: zoomed_in.zoom_percent,
            ..before.clone()
        }
    );

    let _ = click(&mut app, "View/Zoom Out");
    let back = probe(&app);
    assert!(back.zoom_percent < zoomed_in.zoom_percent, "no zoom out");
    assert!((back.zoom_percent - before.zoom_percent).abs() < 0.5);
}

#[test]
fn fit_all_queues_a_fit_to_the_sheet_content() {
    let sheet = sheet_of(vec![symbol(1, "R1", 10.0, 10.0)]);
    let mut app = app_with(sheet.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    // The first-run tour card is showing, as on a fresh install.
    app.ui_state.first_run_tour_open = true;
    let before = probe(&app);
    assert!(!before.fit_queued && before.tour_open);
    let _ = click(&mut app, "View/Fit All");
    // Fit All is not a pointer gesture: the card stays (#1994).
    assert_eq!(
        probe(&app),
        Probe {
            fit_queued: true,
            ..before
        }
    );
}

#[test]
fn toggle_grid_flips_the_visible_grid() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "View/Toggle Grid");
    assert_eq!(
        probe(&app),
        Probe {
            grid_visible: !before.grid_visible,
            ..before.clone()
        }
    );
    let _ = click(&mut app, "View/Toggle Grid");
    assert_eq!(probe(&app), before);
}

#[test]
fn cycle_grid_size_steps_the_snap_grid_and_the_linked_visible_grid() {
    let sizes = crate::canvas::grid::GRID_SIZES_MM;
    let mut app = one_symbol_selected();
    app.ui_state.grid_size_mm = sizes[1];
    app.ui_state.visible_grid_mm = sizes[1];
    let before = probe(&app);
    let _ = click(&mut app, "View/Cycle Grid Size");
    assert_eq!(
        probe(&app),
        Probe {
            grid_size_mm: sizes[2],
            visible_grid_mm: sizes[2],
            ..before
        }
    );
}

#[test]
fn cycle_grid_size_is_greyed_in_a_symbol_editor() {
    assert!(!row_is_live(&symbol_editor_app(), "View/Cycle Grid Size"));
    assert!(row_is_live(&one_symbol_selected(), "View/Cycle Grid Size"));
}

#[test]
fn auto_focus_toggles_the_dimming() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "View/AutoFocus (dim unselected)");
    assert_eq!(
        probe(&app),
        Probe {
            auto_focus: !before.auto_focus,
            ..before
        }
    );
}

#[test]
fn panel_rows_show_their_panel_and_check_it() {
    use crate::panels::PanelKind;
    for (label, kind) in [
        ("Projects", PanelKind::Projects),
        ("Components", PanelKind::Components),
        ("Navigator", PanelKind::Navigator),
        ("Properties", PanelKind::Properties),
        ("ERC", PanelKind::Erc),
        ("Messages", PanelKind::Messages),
        ("Signal", PanelKind::Signal),
    ] {
        let mut app = one_symbol_selected();
        app.document_state.dock.close_panel(kind);
        assert!(!app.is_panel_open(kind), "{label} starts closed");
        let _ = click(&mut app, &format!("View/{label}"));
        assert!(app.is_panel_open(kind), "{label} did not open");
        assert_eq!(
            with_row(&app, &format!("View/{label}"), |item| item.checked),
            Some(true)
        );
    }
}

/// #1922: the menu row saves the grid choice to the preferences file in the
/// background, as Shift+Ctrl+G and the status bar do.
#[test]
fn toggle_grid_row_saves_the_choice() {
    let mut app = one_symbol_selected();
    let task = click(&mut app, "View/Toggle Grid");
    assert!(has_work(&task), "the menu row saved nothing");
}

/// #1922: Zoom In / Out step the schematic or PCB canvas, so they are greyed
/// where there is none: no document, a project alone, and the editors.
#[test]
fn zoom_rows_are_greyed_without_a_canvas() {
    for app in [bare_app(), symbol_editor_app(), footprint_editor_app()] {
        assert!(!row_is_live(&app, "View/Zoom In"));
        assert!(!row_is_live(&app, "View/Zoom Out"));
    }
    let mut project = bare_app();
    open_project(&mut project, "P.snxprj");
    assert!(!row_is_live(&project, "View/Zoom In"));
    let schematic = one_symbol_selected();
    assert!(row_is_live(&schematic, "View/Zoom In"));
    assert!(row_is_live(&schematic, "View/Zoom Out"));
}

/// Fit All on a PCB tab queues a fit to the loaded board.
#[test]
fn fit_all_fits_a_loaded_board() {
    let mut app = loaded_board_app();
    let before = probe(&app);
    assert!(!before.pcb_fit_queued);
    let _ = click(&mut app, "View/Fit All");
    assert_eq!(
        probe(&app),
        Probe {
            pcb_fit_queued: true,
            ..before
        }
    );
}
