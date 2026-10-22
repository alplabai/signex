// Shared private unit-test definitions for line-item outline mode.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_line_outline_tests {
    () => {
        #[cfg(test)]
        mod line_outline_tests {
            use super::*;

            #[test]
            fn toggle_switches_line_render_mode_and_requests_redraw() {
                let mut state = GerberViewerState::default();
                let generation = state.redraw_generation;

                assert_eq!(line_render_mode(state.sketch_lines), LineRenderMode::Filled);

                state.toggle_sketch_lines();

                assert!(state.sketch_lines);
                assert_eq!(
                    line_render_mode(state.sketch_lines),
                    LineRenderMode::Outline
                );
                assert_eq!(state.redraw_generation, generation + 1);
            }

            #[test]
            fn outline_preserves_aperture_width_and_hollows_its_interior() {
                assert_eq!(
                    line_stroke_widths(12.0, LineRenderMode::Outline),
                    (12.0, Some(10.0))
                );
                assert_eq!(
                    line_stroke_widths(12.0, LineRenderMode::Filled),
                    (12.0, None)
                );
                assert_eq!(
                    line_stroke_widths(1.0, LineRenderMode::Outline),
                    (1.0, None)
                );
            }
        }
    };
}

pub(crate) use gerber_line_outline_tests;
