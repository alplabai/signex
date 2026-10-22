// Shared private unit-test definitions for Gerber item selection.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_selection_tests
{
    () => {
        #[cfg(test)]
        mod tests
        {
            use std::io::Cursor;

            use super::*;

            fn selectable_layer(name: &str, visible: bool) -> ViewerLayer
            {
                let layer = signex_gerber::load_gerber_reader(
                    name,
                    Cursor::new(
                        b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\n%ADD11C,2.000*%\nD10*\nX0Y0D03*\nD11*\nX0Y0D03*\nM02*\n",
                    ),
                )
                .expect("selectable Gerber layer must load");
                ViewerLayer {
                    layer,
                    visible,
                    color: Color::from_rgb8(33, 150, 243),
                }
            }

            #[test]
            fn hit_test_prefers_topmost_visible_layer_and_primitive()
            {
                let mut layers = vec![
                    selectable_layer("bottom.gbr", true),
                    selectable_layer("top.gbr", true),
                ];
                let point = signex_gerber::Point::default();

                assert_eq!(
                    hit_test_visible_item(&layers, point, 0.0),
                    Some(GerberItemSelection {
                        layer_index: 1,
                        primitive_index: 1,
                    })
                );

                layers[1].visible = false;

                assert_eq!(
                    hit_test_visible_item(&layers, point, 0.0),
                    Some(GerberItemSelection {
                        layer_index: 0,
                        primitive_index: 1,
                    })
                );
            }

            #[test]
            fn hit_test_handles_strokes_flashes_regions_and_drills()
            {
                let stroke = GerberPrimitive::Stroke {
                    start: signex_gerber::Point { x: 0.0, y: 0.0 },
                    end: signex_gerber::Point { x: 10.0, y: 0.0 },
                    width: 1.0,
                    d_code: Some(10),
                    polarity: PrimitivePolarity::Dark,
                };
                let flash = GerberPrimitive::Flash {
                    position: signex_gerber::Point { x: 5.0, y: 5.0 },
                    aperture: ApertureShape::Rectangle {
                        width: 2.0,
                        height: 4.0,
                    },
                    d_code: Some(11),
                    polarity: PrimitivePolarity::Dark,
                };
                let region = GerberPrimitive::Region {
                    points: vec![
                        signex_gerber::Point { x: 0.0, y: 0.0 },
                        signex_gerber::Point { x: 2.0, y: 0.0 },
                        signex_gerber::Point { x: 1.0, y: 2.0 },
                    ],
                    polarity: PrimitivePolarity::Dark,
                };
                let drill = GerberPrimitive::DrillHit {
                    position: signex_gerber::Point { x: 8.0, y: 8.0 },
                    diameter: 1.0,
                    tool: Some(1),
                };

                assert!(primitive_contains(
                    &stroke,
                    signex_gerber::Point { x: 4.0, y: 0.6 },
                    0.1,
                ));
                assert!(primitive_contains(
                    &flash,
                    signex_gerber::Point { x: 5.9, y: 6.9 },
                    0.0,
                ));
                assert!(primitive_contains(
                    &region,
                    signex_gerber::Point { x: 1.0, y: 1.0 },
                    0.0,
                ));
                assert!(primitive_contains(
                    &drill,
                    signex_gerber::Point { x: 8.5, y: 8.0 },
                    0.0,
                ));
            }

            #[test]
            fn selection_survives_view_navigation_and_tracks_layer_removal()
            {
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![
                        selectable_layer("first.gbr", true).layer,
                        selectable_layer("second.gbr", true).layer,
                    ],
                    failures: Vec::new(),
                });
                state.set_selected_item(Some(GerberItemSelection {
                    layer_index: 1,
                    primitive_index: 0,
                }));

                state.zoom_by(1.2);
                state.pan_by(iced::Vector::new(4.0, -3.0));
                state.fit_page();

                assert_eq!(
                    state.selected_item(),
                    Some(GerberItemSelection {
                        layer_index: 1,
                        primitive_index: 0,
                    })
                );

                state.select_layer(0);
                state.clear_current_layer();

                assert_eq!(
                    state.selected_item(),
                    Some(GerberItemSelection {
                        layer_index: 0,
                        primitive_index: 0,
                    })
                );
            }

            #[test]
            fn selected_primitive_uses_visible_selection_color()
            {
                let base = Color::from_rgb8(244, 67, 54);
                let selection = GerberItemSelection {
                    layer_index: 2,
                    primitive_index: 4,
                };

                assert_eq!(
                    selected_primitive_color(base, Some(selection), &[], 2, 4),
                    Color::from_rgb8(255, 235, 59)
                );
                assert_eq!(
                    selected_primitive_color(base, Some(selection), &[], 2, 3),
                    base
                );
            }

            #[test]
            fn rectangular_selection_returns_every_intersecting_visible_item()
            {
                let layers = vec![
                    selectable_layer("visible.gbr", true),
                    selectable_layer("hidden.gbr", false),
                ];
                let selections = hit_test_visible_items_in_bounds(
                    &layers,
                    Bounds {
                        min: signex_gerber::Point { x: -2.0, y: -2.0 },
                        max: signex_gerber::Point { x: 2.0, y: 2.0 },
                    },
                );

                assert_eq!(
                    selections,
                    vec![
                        GerberItemSelection {
                            layer_index: 0,
                            primitive_index: 0,
                        },
                        GerberItemSelection {
                            layer_index: 0,
                            primitive_index: 1,
                        },
                    ],
                );
            }

            #[test]
            fn region_selection_highlights_every_selected_primitive()
            {
                let base = Color::from_rgb8(244, 67, 54);
                let selections = vec![
                    GerberItemSelection {
                        layer_index: 0,
                        primitive_index: 0,
                    },
                    GerberItemSelection {
                        layer_index: 0,
                        primitive_index: 1,
                    },
                ];

                assert_eq!(
                    selected_primitive_color(base, None, &selections, 0, 1),
                    Color::from_rgb8(255, 235, 59),
                );
                assert_eq!(
                    selected_primitive_color(base, None, &selections, 1, 1),
                    base,
                );
            }
        }
    };
}

pub(crate) use gerber_selection_tests;
