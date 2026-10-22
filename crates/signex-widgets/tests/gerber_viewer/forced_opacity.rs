// Shared private unit-test definitions for forced-opacity compositing.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_forced_opacity_tests {
    () => {
        #[cfg(test)]
        mod forced_opacity_tests {
            use super::*;

            fn composite(top: Color, bottom: Color) -> Color {
                let alpha = top.a + bottom.a * (1.0 - top.a);
                Color {
                    r: (top.r * top.a + bottom.r * bottom.a * (1.0 - top.a)) / alpha,
                    g: (top.g * top.a + bottom.g * bottom.a * (1.0 - top.a)) / alpha,
                    b: (top.b * top.a + bottom.b * bottom.a * (1.0 - top.a)) / alpha,
                    a: alpha,
                }
            }

            #[test]
            fn forced_opacity_uses_configured_clamped_alpha() {
                let color = Color::from_rgb8(211, 47, 47);

                assert_eq!(forced_opacity_color(color, false, 0.60), color,);
                assert_eq!(forced_opacity_color(color, true, 0.60).a, 0.60,);
                assert_eq!(forced_opacity_color(color, true, 2.0).a, 1.0,);
            }

            #[test]
            fn forced_opacity_compositing_is_deterministic_and_order_sensitive() {
                let red = forced_opacity_color(Color::from_rgb8(211, 47, 47), true, 0.60);
                let blue = forced_opacity_color(Color::from_rgb8(33, 150, 243), true, 0.60);

                assert_eq!(composite(red, blue), composite(red, blue));
                assert_ne!(composite(red, blue), composite(blue, red));
                assert_ne!(composite(red, blue), red);
                assert_ne!(composite(red, blue), blue);
            }

            #[test]
            fn forced_opacity_toggle_is_exclusive_with_other_layer_modes() {
                let mut state = GerberViewerState::default();
                assert_eq!(state.forced_opacity, 0.60);
                state.compare_mode = true;
                state.dim_inactive_layers = true;
                let generation = state.redraw_generation;

                state.toggle_forced_opacity_mode();

                assert!(state.forced_opacity_mode);
                assert!(!state.compare_mode);
                assert!(!state.dim_inactive_layers);
                assert_eq!(state.redraw_generation, generation + 1);

                state.toggle_compare_mode();
                assert!(state.compare_mode);
                assert!(!state.forced_opacity_mode);

                state.toggle_forced_opacity_mode();
                state.toggle_dim_inactive_layers();
                assert!(state.dim_inactive_layers);
                assert!(!state.forced_opacity_mode);
            }
        }
    };
}

pub(crate) use gerber_forced_opacity_tests;
