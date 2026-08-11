// Shared private unit-test definitions for clearing Gerber highlights.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_clear_highlight_tests {
    () => {
        #[cfg(test)]
        mod clear_highlight_tests {
            use std::io::Cursor;

            use super::*;

            #[test]
            fn clear_highlight_resets_every_selector_and_preserves_layer_state() {
                let layer = signex_gerber::load_gerber_reader(
                    "layer.gbr",
                    Cursor::new(b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n"),
                )
                .expect("test Gerber must parse");
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![layer],
                    failures: Vec::new(),
                });
                state.highlighted_component = Some("U1".to_owned());
                state.highlighted_net = Some("GND".to_owned());
                state.highlighted_attribute = Some(GerberAttributeValue {
                    name: "AperFunction".to_owned(),
                    values: vec!["ViaPad".to_owned()],
                });
                state.highlighted_d_code = Some(10);
                state.selected_item = Some(GerberItemSelection {
                    layer_index: 0,
                    primitive_index: 0,
                });
                let active_layer = state.active_layer;
                let visible = state.layers[0].visible;
                let generation = state.redraw_generation;

                state.clear_highlight();

                assert!(!state.has_active_highlight());
                assert_eq!(state.highlighted_component, None);
                assert_eq!(state.highlighted_net, None);
                assert_eq!(state.highlighted_attribute, None);
                assert_eq!(state.highlighted_d_code, None);
                assert_eq!(state.active_layer, active_layer);
                assert_eq!(state.layers[0].visible, visible);
                assert!(state.selected_item.is_some());
                assert_eq!(state.redraw_generation, generation + 1);
            }

            #[test]
            fn clearing_without_a_highlight_is_a_no_op() {
                let mut state = GerberViewerState::default();
                let status = state.status.clone();
                let generation = state.redraw_generation;

                state.clear_highlight();

                assert_eq!(state.status, status);
                assert_eq!(state.redraw_generation, generation);
            }

            #[test]
            fn clear_highlight_icon_is_a_black_path_on_a_transparent_background() {
                let source = std::str::from_utf8(highlight_controls::CLEAR_HIGHLIGHT_ICON_ASSET)
                    .expect("clear-highlight SVG must be UTF-8");

                assert!(source.starts_with("<svg "));
                assert!(source.contains("<path "));
                assert!(source.contains("fill=\"#000000\""));
                assert!(!source.contains("<rect"));
                assert!(!source.contains("<circle"));
                assert!(!source.contains("<image"));
                assert!(!source.contains("background"));
            }
        }
    };
}

pub(crate) use gerber_clear_highlight_tests;
