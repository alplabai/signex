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
    let before = probe(&app);
    assert!(!before.fit_queued);
    let _ = click(&mut app, "View/Fit All");
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

/// KNOWN-DIVERGENT (#1922): Shift+Ctrl+G and the status bar save the grid
/// choice to the preferences file in the background; the menu row flips the
/// grid and saves nothing.
#[test]
fn toggle_grid_row_does_not_save_the_choice() {
    let mut app = one_symbol_selected();
    let task = click(&mut app, "View/Toggle Grid");
    assert!(!has_work(&task), "the menu row started a job");
}
