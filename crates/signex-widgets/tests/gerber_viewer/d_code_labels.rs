// Shared private unit-test definitions for viewport D-code labels.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_d_code_labels_tests {
    () => {
        #[cfg(test)]
        mod d_code_labels_tests {
            use super::*;

            #[test]
            fn labels_project_flash_and_stroke_d_codes_to_useful_anchors() {
                let stroke = GerberPrimitive::Stroke {
                    start: signex_gerber::Point { x: 2.0, y: 4.0 },
                    end: signex_gerber::Point { x: 6.0, y: 8.0 },
                    width: 1.0,
                    d_code: Some(10),
                    polarity: PrimitivePolarity::Dark,
                };
                let flash = GerberPrimitive::Flash {
                    position: signex_gerber::Point { x: 9.0, y: 3.0 },
                    aperture: ApertureShape::Circle { diameter: 1.0 },
                    d_code: Some(11),
                    polarity: PrimitivePolarity::Dark,
                };

                assert_eq!(
                    d_code_label(&stroke),
                    Some(DCodeLabel {
                        content: "D10".to_owned(),
                        anchor: signex_gerber::Point { x: 4.0, y: 6.0 },
                    })
                );
                assert_eq!(
                    d_code_label(&flash),
                    Some(DCodeLabel {
                        content: "D11".to_owned(),
                        anchor: signex_gerber::Point { x: 9.0, y: 3.0 },
                    })
                );
            }

            #[test]
            fn labels_are_absent_when_disabled_or_zoomed_too_far_out() {
                assert!(!d_code_labels_visible(false, 4.0));
                assert!(!d_code_labels_visible(true, 0.5));
                assert!(d_code_labels_visible(true, 0.75));

                let region = GerberPrimitive::Region {
                    points: vec![
                        signex_gerber::Point { x: 0.0, y: 0.0 },
                        signex_gerber::Point { x: 1.0, y: 0.0 },
                        signex_gerber::Point { x: 0.0, y: 1.0 },
                    ],
                    polarity: PrimitivePolarity::Dark,
                };
                assert_eq!(d_code_label(&region), None);
            }

            #[test]
            fn toggle_updates_visibility_and_redraw_generation() {
                let mut state = GerberViewerState::default();
                let generation = state.redraw_generation;

                state.toggle_d_code_labels();

                assert!(state.show_d_code_labels);
                assert_eq!(state.redraw_generation, generation + 1);
                assert_eq!(state.d_code_color, Color::from_rgb8(250, 250, 250));
            }
        }
    };
}

pub(crate) use gerber_d_code_labels_tests;
