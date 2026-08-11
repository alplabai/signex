// Shared private unit-test definitions for Gerber component highlighting.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_highlight_tests
{
    () => {
        #[cfg(test)]
        mod tests
        {
            use std::io::Cursor;

            use super::*;

            fn component_layer() -> LoadedLayer
            {
                signex_gerber::load_gerber_reader(
                    "components.gbr",
                    Cursor::new(
                        b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\n%TO.C,R1*%\nX0Y0D03*\n%TO.C,C2*%\nX1000000Y0D03*\nM02*\n",
                    ),
                )
                .expect("X2 component layer must load")
            }

            #[test]
            fn component_choices_follow_the_active_layer_and_clear_stale_selection()
            {
                let first = component_layer();
                let mut second = first.clone();
                second.name = "second.gbr".into();
                for attributes in &mut second.geometry.primitive_attributes
                {
                    attributes.component = Some("U1".into());
                }
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![first, second],
                    failures: Vec::new(),
                });
                state.select_layer(0);

                assert_eq!(state.component_choices(), ["C2", "R1"]);
                state.set_highlighted_component("R1".into());

                state.select_layer(1);

                assert_eq!(state.component_choices(), ["U1"]);
                assert_eq!(state.highlighted_component(), None);
            }

            #[test]
            fn selecting_and_clearing_component_changes_only_highlight_state()
            {
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![component_layer()],
                    failures: Vec::new(),
                });
                let active_layer = state.active_layer;
                let visibility = state.layers[0].visible;

                state.set_highlighted_component("R1".into());

                assert_eq!(state.highlighted_component(), Some("R1"));
                assert_eq!(state.active_layer, active_layer);
                assert_eq!(state.layers[0].visible, visibility);

                state.clear_component_highlight();

                assert_eq!(state.highlighted_component(), None);
                assert_eq!(state.active_layer, active_layer);
                assert_eq!(state.layers[0].visible, visibility);
            }

            #[test]
            fn component_color_highlights_matches_and_dims_other_items()
            {
                let layer_color = Color::from_rgb8(33, 150, 243);
                let matching = signex_gerber::GerberObjectAttributes {
                    component: Some("R1".into()),
                    ..Default::default()
                };
                let other = signex_gerber::GerberObjectAttributes {
                    component: Some("C2".into()),
                    ..Default::default()
                };

                let highlighted = component_highlight_color(
                    layer_color,
                    Some(&matching),
                    Some("R1"),
                );
                let dimmed = component_highlight_color(
                    layer_color,
                    Some(&other),
                    Some("R1"),
                );

                assert_eq!(highlighted, Color::from_rgb8(255, 193, 7));
                assert_eq!(dimmed, Color { a: 0.18, ..layer_color });
                assert_eq!(
                    component_highlight_color(layer_color, Some(&matching), None),
                    layer_color
                );
            }
        }
    };
}

pub(crate) use gerber_highlight_tests;
