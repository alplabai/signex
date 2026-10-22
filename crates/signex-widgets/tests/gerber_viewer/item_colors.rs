// Shared private unit-test definitions for Gerber item colors.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_item_color_tests {
    () => {
        #[cfg(test)]
        mod item_color_tests {
            use super::*;

            #[test]
            fn bundled_item_colors_are_theme_independent_material_colors() {
                let state = GerberViewerState::default();

                assert_eq!(state.grid_color, Color::from_rgb8(117, 117, 117));
                assert_eq!(state.negative_ghost_color, Color::from_rgb8(117, 117, 117),);
                assert_eq!(state.d_code_color, Color::from_rgb8(250, 250, 250),);
                assert!(state.selected_color_choice(state.grid_color).is_some());
                assert!(
                    state
                        .selected_color_choice(state.negative_ghost_color)
                        .is_some()
                );
                assert!(state.selected_color_choice(state.d_code_color).is_some());
            }

            #[test]
            fn item_color_controls_update_their_render_inputs_immediately() {
                let mut state = GerberViewerState::default();
                let alternate_index = state
                    .palette
                    .iter()
                    .position(|material_color| {
                        material_color.color != state.grid_color
                            && material_color.color != state.d_code_color
                    })
                    .expect("alternate Material color");
                let expected = state.palette[alternate_index].color;
                let generation = state.redraw_generation;

                state.set_grid_color(alternate_index);
                state.set_d_code_color(alternate_index);
                state.set_negative_object_color(alternate_index);

                assert_eq!(state.grid_color, expected);
                assert_eq!(state.d_code_color, expected);
                assert_eq!(state.negative_ghost_color, expected);
                assert_eq!(state.redraw_generation, generation + 3);
            }

            #[test]
            fn invalid_and_unchanged_item_colors_are_no_ops() {
                let mut state = GerberViewerState::default();
                let generation = state.redraw_generation;
                let grid_index = state
                    .palette
                    .iter()
                    .position(|material_color| material_color.color == state.grid_color)
                    .expect("grid color in Material palette");
                let d_code_index = state
                    .palette
                    .iter()
                    .position(|material_color| material_color.color == state.d_code_color)
                    .expect("D-code color in Material palette");

                state.set_grid_color(grid_index);
                state.set_d_code_color(d_code_index);
                state.set_negative_object_color(usize::MAX);

                assert_eq!(state.redraw_generation, generation);
            }
        }
    };
}

pub(crate) use gerber_item_color_tests;
