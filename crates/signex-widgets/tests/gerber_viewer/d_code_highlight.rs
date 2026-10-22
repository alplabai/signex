// Shared private unit-test definitions for Gerber D-code highlighting.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_d_code_highlight_tests
{
    () => {
        #[cfg(test)]
        mod d_code_tests
        {
            use std::io::Cursor;

            use super::*;

            fn d_code_layer(name: &str) -> LoadedLayer
            {
                signex_gerber::load_gerber_reader(
                    name,
                    Cursor::new(
                        b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\n%ADD11R,2.000X1.000*%\nD10*\nX0Y0D03*\nD11*\nX1000000Y0D03*\nM02*\n",
                    ),
                )
                .expect("D-code layer must load")
            }

            #[test]
            fn active_layer_exposes_defined_d_codes_with_usage()
            {
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![d_code_layer("copper.gbr")],
                    failures: Vec::new(),
                });

                let choices = state.d_code_choices();

                assert_eq!(
                    choices.iter().map(|choice| choice.code).collect::<Vec<_>>(),
                    [10, 11]
                );
                assert!(choices[0].to_string().starts_with("D10 — circle"));
                assert!(choices[0].to_string().ends_with("(1 item)"));
            }

            #[test]
            fn selection_is_exclusive_and_clears_when_active_layer_changes()
            {
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![
                        d_code_layer("top.gbr"),
                        d_code_layer("bottom.gbr"),
                    ],
                    failures: Vec::new(),
                });
                state.select_layer(0);
                state.highlighted_component = Some("R1".into());
                state.highlighted_net = Some("GND".into());
                state.highlighted_attribute = Some(GerberAttributeValue {
                    name: ".CVal".into(),
                    values: vec!["10k".into()],
                });

                state.set_highlighted_d_code(11);

                assert_eq!(state.highlighted_d_code(), Some(11));
                assert_eq!(state.highlighted_component(), None);
                assert_eq!(state.highlighted_net(), None);
                assert_eq!(state.highlighted_attribute(), None);

                state.select_layer(1);

                assert_eq!(state.highlighted_d_code(), None);
            }

            #[test]
            fn unavailable_d_code_selection_is_ignored()
            {
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![d_code_layer("copper.gbr")],
                    failures: Vec::new(),
                });

                state.set_highlighted_d_code(99);

                assert_eq!(state.highlighted_d_code(), None);
            }

            #[test]
            fn d_code_color_highlights_exact_matches_and_dims_other_items()
            {
                let layer_color = Color::from_rgb8(0, 188, 212);
                let matching = GerberPrimitive::Flash {
                    position: signex_gerber::Point::default(),
                    aperture: ApertureShape::Circle { diameter: 1.0 },
                    d_code: Some(10),
                    polarity: PrimitivePolarity::Dark,
                };
                let other = GerberPrimitive::Flash {
                    position: signex_gerber::Point::default(),
                    aperture: ApertureShape::Circle { diameter: 1.0 },
                    d_code: Some(11),
                    polarity: PrimitivePolarity::Dark,
                };

                assert_eq!(
                    d_code_highlight_color(layer_color, &matching, Some(10)),
                    Color::from_rgb8(255, 193, 7)
                );
                assert_eq!(
                    d_code_highlight_color(layer_color, &other, Some(10)),
                    Color { a: 0.18, ..layer_color }
                );
                assert_eq!(
                    d_code_highlight_color(layer_color, &matching, None),
                    layer_color
                );
            }
        }
    };
}

pub(crate) use gerber_d_code_highlight_tests;
