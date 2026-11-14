//! #1672 - `fast_grid_1`, `fast_grid_2` and `cycle_fast_grid` set the snap
//! grid instead of reporting "not available yet".

use signex_app::app::Signex;
use signex_app::canvas::grid::FAST_GRIDS_MM;

use crate::menu_support::menu_row;

fn app_on_grid(mm: f32) -> Signex {
    let (mut app, _t) = Signex::new();
    app.ui_state.grid_size_mm = mm;
    app.ui_state.visible_grid_mm = mm;
    app
}

#[test]
fn fast_grid_1_and_2_snap_to_their_grids() {
    let mut app = app_on_grid(5.08);

    let _ = app.update(menu_row("fast_grid_1"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[0]);
    assert_eq!(app.ui_state.visible_grid_mm, FAST_GRIDS_MM[0]);

    let _ = app.update(menu_row("fast_grid_2"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[1]);
    assert_eq!(app.ui_state.visible_grid_mm, FAST_GRIDS_MM[1]);
}

#[test]
fn cycle_fast_grid_steps_between_the_two_and_wraps() {
    let mut app = app_on_grid(FAST_GRIDS_MM[0]);

    let _ = app.update(menu_row("cycle_fast_grid"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[1]);

    let _ = app.update(menu_row("cycle_fast_grid"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[0]);
}

#[test]
fn cycle_fast_grid_from_another_grid_lands_on_fast_grid_1() {
    let mut app = app_on_grid(0.635);

    let _ = app.update(menu_row("cycle_fast_grid"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[0]);
}

#[test]
fn a_fast_grid_leaves_a_deliberately_different_visible_grid_alone() {
    let mut app = app_on_grid(0.635);
    app.ui_state.visible_grid_mm = 5.08;

    let _ = app.update(menu_row("fast_grid_2"));
    assert_eq!(app.ui_state.grid_size_mm, FAST_GRIDS_MM[1]);
    assert_eq!(app.ui_state.visible_grid_mm, 5.08);
}
