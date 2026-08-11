// Shared private unit-test definitions for inactive-layer dimming.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_dim_inactive_layers_tests {
    () => {
        #[cfg(test)]
        mod dim_inactive_layers_tests {
            use super::*;

            #[test]
            fn active_and_normal_layers_preserve_their_color() {
                let color = Color {
                    a: 0.8,
                    ..Color::from_rgb8(211, 47, 47)
                };

                assert_eq!(inactive_layer_color(color, true, true, 0.28), color);
                assert_eq!(inactive_layer_color(color, false, false, 0.28), color);
            }

            #[test]
            fn inactive_layer_uses_configured_clamped_opacity() {
                let color = Color {
                    a: 0.8,
                    ..Color::from_rgb8(211, 47, 47)
                };
                let dimmed = inactive_layer_color(color, false, true, 0.28);

                assert_eq!(dimmed.r, color.r);
                assert_eq!(dimmed.g, color.g);
                assert_eq!(dimmed.b, color.b);
                assert_eq!(dimmed.a, 0.8 * 0.28);
                assert_eq!(inactive_layer_color(color, false, true, 2.0).a, color.a);
            }

            #[test]
            fn bundled_opacity_and_toggle_drive_redraw_state() {
                let mut state = GerberViewerState::default();
                let generation = state.redraw_generation;

                assert_eq!(state.inactive_layer_opacity, 0.28);

                state.toggle_dim_inactive_layers();

                assert!(state.dim_inactive_layers);
                assert_eq!(state.redraw_generation, generation + 1);
            }

            #[test]
            fn dim_and_compare_modes_are_mutually_exclusive() {
                let mut state = GerberViewerState::default();

                state.toggle_compare_mode();
                state.toggle_dim_inactive_layers();

                assert!(state.dim_inactive_layers);
                assert!(!state.compare_mode);

                state.toggle_compare_mode();

                assert!(state.compare_mode);
                assert!(!state.dim_inactive_layers);
            }
        }
    };
}

pub(crate) use gerber_dim_inactive_layers_tests;
