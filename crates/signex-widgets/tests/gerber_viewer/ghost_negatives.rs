// Shared private unit-test definitions for negative-object ghost rendering.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_ghost_negatives_tests {
    () => {
        #[cfg(test)]
        mod ghost_negatives_tests {
            use super::*;

            #[test]
            fn disabled_mode_preserves_normal_polarity_compositing() {
                let dark = Color::from_rgb8(211, 47, 47);
                let background = Color::from_rgb8(20, 20, 20);
                let ghost = Color::from_rgb8(117, 117, 117);

                assert_eq!(
                    primitive_polarity_color(
                        PrimitivePolarity::Dark,
                        dark,
                        background,
                        ghost,
                        false,
                    ),
                    dark
                );
                assert_eq!(
                    primitive_polarity_color(
                        PrimitivePolarity::Clear,
                        dark,
                        background,
                        ghost,
                        false,
                    ),
                    background
                );
            }

            #[test]
            fn enabled_mode_uses_configured_material_ghost_color() {
                let mut state = GerberViewerState::default();
                let generation = state.redraw_generation;

                state.toggle_ghost_negative_objects();

                assert!(state.ghost_negative_objects);
                assert_eq!(
                    primitive_polarity_color(
                        PrimitivePolarity::Clear,
                        Color::BLACK,
                        Color::WHITE,
                        state.negative_ghost_color,
                        true,
                    ),
                    Color::from_rgb8(117, 117, 117)
                );
                assert_eq!(state.redraw_generation, generation + 1);
            }
        }
    };
}

pub(crate) use gerber_ghost_negatives_tests;
