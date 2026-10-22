// Shared private unit-test definitions for polygon-item outline mode.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_polygon_outline_tests {
    () => {
        #[cfg(test)]
        mod polygon_outline_tests {
            use super::*;

            #[test]
            fn toggle_switches_polygon_render_mode_and_requests_redraw() {
                let mut state = GerberViewerState::default();
                let generation = state.redraw_generation;

                assert_eq!(
                    polygon_render_mode(state.sketch_polygons),
                    PolygonRenderMode::Filled
                );

                state.toggle_sketch_polygons();

                assert!(state.sketch_polygons);
                assert_eq!(
                    polygon_render_mode(state.sketch_polygons),
                    PolygonRenderMode::Outline
                );
                assert_eq!(state.redraw_generation, generation + 1);

                state.toggle_sketch_polygons();

                assert_eq!(
                    polygon_render_mode(state.sketch_polygons),
                    PolygonRenderMode::Filled
                );
            }
        }
    };
}

pub(crate) use gerber_polygon_outline_tests;
