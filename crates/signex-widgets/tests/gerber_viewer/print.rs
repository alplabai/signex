// Shared private unit-test definitions for Gerber printing.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_print_tests {
    () => {
        #[cfg(test)]
        mod tests {
            use std::io::Cursor;

            use super::*;
            use crate::gerber_viewer::GerberViewerState;
            use iced::Color;
            use signex_gerber::GerberLoadBatch;

            fn two_layer_state() -> GerberViewerState {
                let first = signex_gerber::load_gerber_reader(
                    "visible.gbr",
                    Cursor::new(b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n"),
                )
                .expect("visible Gerber");
                let second = signex_gerber::load_excellon_reader(
                    "hidden.drl",
                    Cursor::new(b"M48\nMETRIC\nT01C0.8\n%\nG05\nT01\nX1.0Y1.0\nM30\n"),
                )
                .expect("hidden drill");
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![first, second],
                    failures: Vec::new(),
                });
                state.layers[0].color = Color::from_rgb8(244, 67, 54);
                state.layers[1].visible = false;
                state
            }

            #[test]
            fn print_plan_contains_only_visible_layers_and_preserves_colors() {
                let state = two_layer_state();

                let plan = build_plan(&state).expect("print plan");

                assert_eq!(plan.layers.len(), 1);
                assert_eq!(plan.layers[0].name, "visible.gbr");
                assert_eq!(
                    plan.layers[0].color,
                    [244.0 / 255.0, 67.0 / 255.0, 54.0 / 255.0]
                );
                assert_eq!(plan.page_size, GerberPageSize::FullSize);
                assert!(plan.content_scale > 0.0);
            }

            #[test]
            fn fixed_page_plan_respects_page_size_and_printable_margin() {
                let mut state = two_layer_state();
                state.page_size = GerberPageSize::A4;

                let plan = build_plan(&state).expect("print plan");

                assert_eq!((plan.page_width_mm, plan.page_height_mm), (297.0, 210.0));
                assert_eq!(plan.printable_bounds_mm.min, Point { x: 10.0, y: 10.0 });
                assert_eq!(plan.printable_bounds_mm.max, Point { x: 287.0, y: 200.0 });
                assert!(plan.content_scale < 1.0);
            }

            #[test]
            fn pdf_is_vector_page_with_clipping_and_no_hidden_layer_color() {
                let state = two_layer_state();

                let bytes = build_pdf(&state).expect("Gerber PDF");
                let text = String::from_utf8_lossy(&bytes);

                assert!(bytes.starts_with(b"%PDF-"));
                assert!(text.contains("/MediaBox"));
                assert!(text.contains("W n"));
                assert!(text.contains("0.95686275 0.2627451 0.21176471 rg"));
                assert!(!text.contains("hidden.drl"));
            }

            #[test]
            fn empty_viewer_cannot_build_a_print_plan() {
                let state = GerberViewerState::default();

                assert!(build_plan(&state).is_err());
            }
        }
    };
}

pub(crate) use gerber_print_tests;
