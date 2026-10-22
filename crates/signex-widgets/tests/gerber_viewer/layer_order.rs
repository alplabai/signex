// Shared private unit-test definitions for Gerber layer ordering.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_layer_order_tests {
    () => {
        #[cfg(test)]
        mod layer_order_tests {
            use std::io::Cursor;

            use super::*;

            fn three_layer_state() -> GerberViewerState {
                let layer = signex_gerber::load_gerber_reader(
                    "layer.gbr",
                    Cursor::new(b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n"),
                )
                .expect("test Gerber must parse");
                let mut first = layer.clone();
                first.name = "first.gbr".into();
                let mut second = layer.clone();
                second.name = "second.gbr".into();
                let mut third = layer;
                third.name = "third.gbr".into();
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![first, second, third],
                    failures: Vec::new(),
                });
                state
            }

            #[test]
            fn moving_active_layer_changes_render_order_and_preserves_layer_state() {
                let mut state = three_layer_state();
                state.select_layer(1);
                state.set_layer_visible(1, false);
                let active_color = state.layers[1].color;
                state.selected_item = Some(GerberItemSelection {
                    layer_index: 1,
                    primitive_index: 0,
                });
                let generation = state.redraw_generation;

                state.move_active_layer_up();

                assert_eq!(
                    state
                        .layers
                        .iter()
                        .map(|layer| layer.layer.name.as_str())
                        .collect::<Vec<_>>(),
                    vec!["first.gbr", "third.gbr", "second.gbr"],
                );
                assert_eq!(state.active_layer, Some(2));
                assert!(!state.layers[2].visible);
                assert_eq!(state.layers[2].color, active_color);
                assert_eq!(
                    state.selected_item(),
                    Some(GerberItemSelection {
                        layer_index: 2,
                        primitive_index: 0,
                    }),
                );
                assert_eq!(state.redraw_generation, generation + 1);

                state.move_active_layer_down();

                assert_eq!(
                    state
                        .layers
                        .iter()
                        .map(|layer| layer.layer.name.as_str())
                        .collect::<Vec<_>>(),
                    vec!["first.gbr", "second.gbr", "third.gbr"],
                );
                assert_eq!(state.active_layer, Some(1));
                assert_eq!(
                    state.selected_item(),
                    Some(GerberItemSelection {
                        layer_index: 1,
                        primitive_index: 0,
                    }),
                );
            }

            #[test]
            fn layer_order_boundaries_are_no_ops() {
                let mut state = three_layer_state();
                state.select_layer(2);
                let generation = state.redraw_generation;

                state.move_active_layer_up();

                assert_eq!(state.layers[2].layer.name, "third.gbr");
                assert_eq!(state.active_layer, Some(2));
                assert_eq!(state.redraw_generation, generation);

                state.select_layer(0);
                state.move_active_layer_down();

                assert_eq!(state.layers[0].layer.name, "first.gbr");
                assert_eq!(state.active_layer, Some(0));
                assert_eq!(state.redraw_generation, generation);
            }

            #[test]
            fn layer_control_icons_are_black_paths_on_transparent_backgrounds() {
                for asset in layer_controls::LAYER_CONTROL_ICON_ASSETS {
                    let source =
                        std::str::from_utf8(asset).expect("layer-control SVG must be UTF-8");

                    assert!(source.starts_with("<svg "));
                    assert!(source.contains("<path "));
                    assert!(source.contains("fill=\"#000000\""));
                    assert!(!source.contains("<image"));
                    assert!(!source.contains("background"));
                }
            }
        }
    };
}

pub(crate) use gerber_layer_order_tests;
