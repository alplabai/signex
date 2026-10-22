// Shared private unit-test definitions for the Gerber viewer.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_grid_state_tests {
    () => {
        #[cfg(test)]
        mod grid_state_tests {
            use super::*;

            #[test]
            fn selecting_grid_updates_rectangular_viewport_spacing() {
                let mut state = GerberViewerState::default();
                state.decimal_separator = ".".to_owned();
                let initial_generation = state.redraw_generation;

                assert_eq!(state.active_grid_index, DEFAULT_GRID_INDEX);
                assert_eq!(state.active_grid().x_millimetres(), 1.27);
                assert_eq!(state.active_grid().y_millimetres(), 1.27);

                state.select_grid_size(13);

                assert_eq!(state.active_grid_index, 13);
                assert_eq!(state.active_grid().x_millimetres(), 1.5);
                assert_eq!(state.active_grid().y_millimetres(), 2.5);
                assert_eq!(state.redraw_generation, initial_generation + 1);
                assert_eq!(
                    state.status,
                    "Grid: 1.5000 mm ⨯ 2.5000 mm (59.06 mils ⨯ 98.43 mils)"
                );
            }

            #[test]
            fn toggling_grid_visibility_only_requests_viewport_redraw() {
                let mut state = GerberViewerState::default();
                let grid_catalog = state.grid_catalog.clone();
                let active_grid_index = state.active_grid_index;
                let layers = state.layers.len();
                let initial_generation = state.redraw_generation;

                state.set_grid_visible(false);

                assert!(!state.grid_visible);
                assert_eq!(state.redraw_generation, initial_generation + 1);
                assert_eq!(state.grid_catalog, grid_catalog);
                assert_eq!(state.active_grid_index, active_grid_index);
                assert_eq!(state.layers.len(), layers);

                state.set_grid_visible(false);
                assert_eq!(state.redraw_generation, initial_generation + 1);

                state.set_grid_visible(true);
                assert!(state.grid_visible);
                assert_eq!(state.redraw_generation, initial_generation + 2);
            }

            #[test]
            fn creating_grid_appends_selects_and_persists_definition() {
                let mut state = GerberViewerState::default();
                state.decimal_separator = ".".to_owned();
                state.set_new_grid_name(" Fine metric ".to_owned());
                state.set_new_grid_x("0.05".to_owned());
                state.set_new_grid_y("0".to_owned());
                state.set_new_grid_unit_millimetres(true);
                let original_count = state.grid_catalog.len();
                let mut persisted = Vec::new();

                state.create_grid_with(|catalog| {
                    persisted = catalog.to_vec();
                    Ok(())
                });

                assert_eq!(state.grid_catalog.len(), original_count + 1);
                assert_eq!(persisted, state.grid_catalog);
                assert_eq!(state.active_grid_index, original_count);
                assert_eq!(state.active_grid().name.as_deref(), Some("Fine metric"));
                assert_eq!(state.active_grid().x_millimetres(), 0.05);
                assert_eq!(state.active_grid().y_millimetres(), 0.0);
                assert_eq!(
                    state.status,
                    "Created grid: Fine metric: 0.0500 mm ⨯ 0.0000 mm (1.97 mils ⨯ 0.00 mils)"
                );
                assert!(state.new_grid_name.is_empty());
                assert!(state.new_grid_x.is_empty());
                assert!(state.new_grid_y.is_empty());
                assert_eq!(state.grid_editor_error, None);
            }

            #[test]
            fn invalid_or_unpersisted_grid_is_not_added() {
                let mut state = GerberViewerState::default();
                let original_catalog = state.grid_catalog.clone();
                state.set_new_grid_x("0".to_owned());
                state.set_new_grid_y("1".to_owned());

                state.create_grid_with(|_| panic!("invalid grid must not persist"));

                assert_eq!(state.grid_catalog, original_catalog);
                assert_eq!(
                    state.grid_editor_error.as_deref(),
                    Some("X must be a finite number greater than zero.")
                );

                state.set_new_grid_x("1".to_owned());
                state.create_grid_with(|_| Err("settings unavailable".to_owned()));

                assert_eq!(state.grid_catalog, original_catalog);
                assert_eq!(
                    state.grid_editor_error.as_deref(),
                    Some("settings unavailable")
                );
            }

            #[test]
            fn editing_grid_updates_same_entry_and_persists_catalog() {
                let mut state = GerberViewerState::default();
                state.grid_catalog = grid::default_grid_catalog()[0..3].to_vec();
                state.active_grid_index = 1;
                state.decimal_separator = ".".to_owned();
                state.load_active_grid_into_editor();
                let original_count = state.grid_catalog.len();
                let initial_generation = state.redraw_generation;
                state.set_edit_grid_name(" Fine metric ".to_owned());
                state.set_edit_grid_unit_millimetres(true);
                state.set_edit_grid_x("0.05".to_owned());
                state.set_edit_grid_y("0.10".to_owned());
                let mut persisted = Vec::new();

                state.update_grid_with(|catalog| {
                    persisted = catalog.to_vec();
                    Ok(())
                });

                assert_eq!(state.grid_catalog, persisted);
                assert_eq!(state.grid_catalog.len(), original_count);
                assert_eq!(state.active_grid_index, 1);
                assert_eq!(state.active_grid().name.as_deref(), Some("Fine metric"));
                assert_eq!(state.active_grid().x_millimetres(), 0.05);
                assert_eq!(state.active_grid().y_millimetres(), 0.10);
                assert_eq!(state.active_grid().unit, GridUnit::Mm);
                assert_eq!(state.redraw_generation, initial_generation + 1);
                assert_eq!(
                    state.status,
                    "Updated grid: Fine metric: 0.0500 mm ⨯ 0.1000 mm (1.97 mils ⨯ 3.94 mils)"
                );
                assert_eq!(state.edit_grid_name, "Fine metric");
                assert_eq!(state.edit_grid_x, "0.05");
                assert_eq!(state.edit_grid_y, "0.1");
                assert_eq!(state.grid_editor_error, None);
            }

            #[test]
            fn changing_edit_unit_preserves_physical_grid_spacing() {
                let mut state = GerberViewerState::default();
                state.grid_catalog = grid::default_grid_catalog()[0..2].to_vec();
                state.active_grid_index = 0;
                state.decimal_separator = ".".to_owned();
                state.load_active_grid_into_editor();
                let original_x = state.active_grid().x_millimetres();
                let original_y = state.active_grid().y_millimetres();

                state.set_edit_grid_unit_millimetres(true);

                assert_eq!(state.edit_grid_unit, GridUnit::Mm);
                assert_eq!(state.edit_grid_x, "2.54");
                assert_eq!(state.edit_grid_y, "2.54");
                let metric = create_grid_definition(
                    "",
                    &state.edit_grid_x,
                    &state.edit_grid_y,
                    state.edit_grid_unit,
                    &state.decimal_separator,
                )
                .expect("converted metric draft");
                assert_eq!(metric.x_millimetres(), original_x);
                assert_eq!(metric.y_millimetres(), original_y);

                state.set_edit_grid_unit_millimetres(false);

                assert_eq!(state.edit_grid_unit, GridUnit::Mil);
                assert_eq!(state.edit_grid_x, "100");
                assert_eq!(state.edit_grid_y, "100");
            }

            #[test]
            fn invalid_or_unpersisted_grid_edit_does_not_change_catalog() {
                let mut state = GerberViewerState::default();
                state.grid_catalog = grid::default_grid_catalog()[0..3].to_vec();
                state.active_grid_index = 1;
                state.load_active_grid_into_editor();
                let original_catalog = state.grid_catalog.clone();
                state.set_edit_grid_x("0".to_owned());

                state.update_grid_with(|_| panic!("invalid edit must not persist"));

                assert_eq!(state.grid_catalog, original_catalog);
                assert_eq!(
                    state.grid_editor_error.as_deref(),
                    Some("X must be a finite number greater than zero.")
                );

                state.set_edit_grid_x("1".to_owned());
                state.update_grid_with(|_| Err("settings unavailable".to_owned()));

                assert_eq!(state.grid_catalog, original_catalog);
                assert_eq!(state.active_grid_index, 1);
                assert_eq!(
                    state.grid_editor_error.as_deref(),
                    Some("settings unavailable")
                );
            }

            #[test]
            fn deleting_grid_persists_catalog_and_selects_next_entry() {
                let mut state = GerberViewerState::default();
                state.grid_catalog = grid::default_grid_catalog()[0..3].to_vec();
                state.active_grid_index = 1;
                let removed = state.grid_catalog[1].clone();
                let next = state.grid_catalog[2].clone();
                let initial_generation = state.redraw_generation;
                let mut persisted = Vec::new();

                state.delete_grid_with(|catalog| {
                    persisted = catalog.to_vec();
                    Ok(())
                });

                assert_eq!(state.grid_catalog, persisted);
                assert_eq!(state.grid_catalog.len(), 2);
                assert!(!state.grid_catalog.contains(&removed));
                assert_eq!(state.active_grid_index, 1);
                assert_eq!(state.active_grid(), &next);
                assert_eq!(state.redraw_generation, initial_generation + 1);
                assert!(state.status.starts_with("Deleted grid: "));
                assert_eq!(state.grid_editor_error, None);
            }

            #[test]
            fn deleting_final_grid_selects_previous_and_never_empties_catalog() {
                let mut state = GerberViewerState::default();
                state.grid_catalog = grid::default_grid_catalog()[0..2].to_vec();
                state.active_grid_index = 1;
                let previous = state.grid_catalog[0].clone();

                state.delete_grid_with(|_| Ok(()));

                assert_eq!(state.grid_catalog, vec![previous.clone()]);
                assert_eq!(state.active_grid_index, 0);
                assert_eq!(state.active_grid(), &previous);

                state.delete_grid_with(|_| panic!("last grid must not persist"));

                assert_eq!(state.grid_catalog, vec![previous]);
                assert_eq!(
                    state.grid_editor_error.as_deref(),
                    Some("At least one grid definition must remain.")
                );
            }

            #[test]
            fn failed_grid_deletion_does_not_change_catalog_or_selection() {
                let mut state = GerberViewerState::default();
                state.grid_catalog = grid::default_grid_catalog()[0..3].to_vec();
                state.active_grid_index = 1;
                let original_catalog = state.grid_catalog.clone();

                state.delete_grid_with(|_| Err("settings unavailable".to_owned()));

                assert_eq!(state.grid_catalog, original_catalog);
                assert_eq!(state.active_grid_index, 1);
                assert_eq!(
                    state.grid_editor_error.as_deref(),
                    Some("settings unavailable")
                );
            }

            #[test]
            fn moving_grid_up_and_down_preserves_selected_definition() {
                let mut state = GerberViewerState::default();
                state.grid_catalog = grid::default_grid_catalog()[0..3].to_vec();
                state.active_grid_index = 1;
                let original_catalog = state.grid_catalog.clone();
                let selected = state.active_grid().clone();
                let mut persisted = Vec::new();

                state.move_grid_up_with(|catalog| {
                    persisted = catalog.to_vec();
                    Ok(())
                });

                assert_eq!(state.grid_catalog, persisted);
                assert_eq!(state.active_grid_index, 0);
                assert_eq!(state.active_grid(), &selected);
                assert_eq!(state.grid_catalog[1], original_catalog[0]);

                state.move_grid_down_with(|catalog| {
                    persisted = catalog.to_vec();
                    Ok(())
                });

                assert_eq!(state.grid_catalog, original_catalog);
                assert_eq!(state.grid_catalog, persisted);
                assert_eq!(state.active_grid_index, 1);
                assert_eq!(state.active_grid(), &selected);
            }

            #[test]
            fn moving_grid_at_boundary_is_a_no_op() {
                let mut state = GerberViewerState::default();
                state.grid_catalog = grid::default_grid_catalog()[0..3].to_vec();
                let original_catalog = state.grid_catalog.clone();
                let original_generation = state.redraw_generation;

                state.active_grid_index = 0;
                state.move_grid_up_with(|_| panic!("upper boundary must not persist"));
                state.active_grid_index = state.grid_catalog.len() - 1;
                state.move_grid_down_with(|_| panic!("lower boundary must not persist"));

                assert_eq!(state.grid_catalog, original_catalog);
                assert_eq!(state.active_grid_index, 2);
                assert_eq!(state.redraw_generation, original_generation);
            }

            #[test]
            fn failed_grid_move_does_not_change_catalog_or_selection() {
                let mut state = GerberViewerState::default();
                state.grid_catalog = grid::default_grid_catalog()[0..3].to_vec();
                state.active_grid_index = 1;
                let original_catalog = state.grid_catalog.clone();

                state.move_grid_up_with(|_| Err("settings unavailable".to_owned()));

                assert_eq!(state.grid_catalog, original_catalog);
                assert_eq!(state.active_grid_index, 1);
                assert_eq!(
                    state.grid_editor_error.as_deref(),
                    Some("settings unavailable")
                );
            }

            #[test]
            fn zero_grid_axis_is_not_replaced_with_the_other_axis() {
                let mut state = GerberViewerState::default();

                state.select_grid_size(19);

                assert_eq!(state.active_grid().x_millimetres(), 0.05);
                assert_eq!(state.active_grid().y_millimetres(), 0.0);
                assert!(visible_grid_spacing(0.05 * 100.0).is_some());
                assert_eq!(visible_grid_spacing(0.0), None);
            }
        }
    };
}

pub(crate) use gerber_grid_state_tests;
