// Shared private unit-test definitions for Gerber net highlighting.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_net_highlight_tests
{
    () => {
        #[cfg(test)]
        mod net_tests
        {
            use std::io::Cursor;

            use super::*;

            fn net_layer() -> LoadedLayer
            {
                signex_gerber::load_gerber_reader(
                    "nets.gbr",
                    Cursor::new(
                        b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\n%TO.N,GND*%\nX0Y0D03*\n%TO.N,VCC*%\nX1000000Y0D03*\nM02*\n",
                    ),
                )
                .expect("X2 net layer must load")
            }

            #[test]
            fn net_choices_are_sorted_unique_and_selection_is_exclusive()
            {
                let first = net_layer();
                let mut second = first.clone();
                second.name = "second.gbr".into();
                for attributes in &mut second.geometry.primitive_attributes
                {
                    attributes.nets = vec!["SIGNAL".into()];
                }
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![first, second],
                    failures: Vec::new(),
                });
                state.select_layer(0);
                state.highlighted_component = Some("R1".into());

                assert_eq!(state.net_choices(), ["GND", "VCC"]);

                state.set_highlighted_net("VCC".into());

                assert_eq!(state.highlighted_net(), Some("VCC"));
                assert_eq!(state.highlighted_component(), None);

                state.select_layer(1);

                assert_eq!(state.net_choices(), ["SIGNAL"]);
                assert_eq!(state.highlighted_net(), None);
            }

            #[test]
            fn clearing_net_restores_normal_color_without_changing_layers()
            {
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![net_layer()],
                    failures: Vec::new(),
                });
                let active_layer = state.active_layer;
                state.set_highlighted_net("GND".into());

                state.clear_net_highlight();

                assert_eq!(state.highlighted_net(), None);
                assert_eq!(state.active_layer, active_layer);
                assert!(state.layers[0].visible);
            }

            #[test]
            fn net_color_highlights_any_matching_net_and_dims_nonmatches()
            {
                let layer_color = Color::from_rgb8(76, 175, 80);
                let attributes = signex_gerber::GerberObjectAttributes {
                    nets: vec!["GND".into(), "SIGNAL".into()],
                    ..Default::default()
                };

                assert_eq!(
                    net_highlight_color(
                        layer_color,
                        Some(&attributes),
                        Some("GND"),
                    ),
                    Color::from_rgb8(255, 193, 7)
                );
                assert_eq!(
                    net_highlight_color(
                        layer_color,
                        Some(&attributes),
                        Some("VCC"),
                    ),
                    Color { a: 0.18, ..layer_color }
                );
                assert_eq!(
                    net_highlight_color(layer_color, Some(&attributes), None),
                    layer_color
                );
            }
        }
    };
}

pub(crate) use gerber_net_highlight_tests;
