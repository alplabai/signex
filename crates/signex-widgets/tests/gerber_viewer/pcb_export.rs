// Shared private unit-test definitions for lossy native PCB export.
#![allow(unused_imports, unused_macros)]

macro_rules! gerber_pcb_export_tests {
    () => {
        #[cfg(test)]
        mod pcb_export_tests {
            use std::io::Cursor;

            use super::*;

            fn loaded_layer(
                name: &str,
                layer_type: signex_gerber::LayerType,
                primitives: Vec<GerberPrimitive>,
            ) -> LoadedLayer {
                let mut layer = signex_gerber::load_gerber_reader(
                    name,
                    Cursor::new(b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nM02*\n"),
                )
                .expect("test Gerber must parse");
                layer.name = name.to_owned();
                layer.layer_type = layer_type;
                layer.geometry.primitives = primitives;
                layer
            }

            #[test]
            fn converts_supported_dark_copper_lines_and_flashes() {
                let top = loaded_layer(
                    "top.gtl",
                    signex_gerber::LayerType::Top,
                    vec![
                        GerberPrimitive::Stroke {
                            start: signex_gerber::Point { x: 1.0, y: 2.0 },
                            end: signex_gerber::Point { x: 3.0, y: 4.0 },
                            width: 0.25,
                            d_code: Some(10),
                            polarity: PrimitivePolarity::Dark,
                        },
                        GerberPrimitive::Flash {
                            position: signex_gerber::Point { x: 5.0, y: 6.0 },
                            aperture: ApertureShape::Circle { diameter: 1.2 },
                            d_code: Some(11),
                            polarity: PrimitivePolarity::Dark,
                        },
                    ],
                );
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![top],
                    failures: Vec::new(),
                });

                let export = state.export_native_pcb();

                assert!(export.report.lossy);
                assert_eq!(export.report.converted_lines, 1);
                assert_eq!(export.report.converted_flashes, 1);
                assert_eq!(export.report.skipped_count(), 0);
                assert_eq!(export.board.segments[0].layer, "F.Cu");
                assert_eq!(export.board.segments[0].width, 0.25);
                assert_eq!(
                    export.board.footprints[0].pads[0].shape,
                    signex_types::pcb::PadShape::Circle
                );
                assert_eq!(export.board.footprints[0].pads[0].layers, vec!["F.Cu"],);
                let source = export.write_string().expect("native PCB serialization");
                let round_trip =
                    signex_types::format::SnxPcb::parse(&source).expect("native PCB round trip");
                assert_eq!(round_trip.board.segments.len(), 1);
                assert_eq!(round_trip.board.footprints[0].pads.len(), 1);
                assert!(round_trip.board.generator.contains("lossy"));
            }

            #[test]
            fn maps_supported_non_copper_lines_to_board_graphics() {
                let silk = loaded_layer(
                    "top.gto",
                    signex_gerber::LayerType::SilkScreenTop,
                    vec![GerberPrimitive::Stroke {
                        start: signex_gerber::Point { x: 0.0, y: 0.0 },
                        end: signex_gerber::Point { x: 2.0, y: 0.0 },
                        width: 0.15,
                        d_code: None,
                        polarity: PrimitivePolarity::Dark,
                    }],
                );
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![silk],
                    failures: Vec::new(),
                });

                let export = state.export_native_pcb();

                assert_eq!(export.board.graphics.len(), 1);
                assert_eq!(export.board.graphics[0].layer, "F.SilkS");
                assert_eq!(export.report.converted_lines, 1);
            }

            #[test]
            fn reports_unsupported_constructs_instead_of_converting_them() {
                let top = loaded_layer(
                    "top.gtl",
                    signex_gerber::LayerType::Top,
                    vec![
                        GerberPrimitive::Stroke {
                            start: signex_gerber::Point { x: 0.0, y: 0.0 },
                            end: signex_gerber::Point { x: 1.0, y: 0.0 },
                            width: 0.2,
                            d_code: None,
                            polarity: PrimitivePolarity::Clear,
                        },
                        GerberPrimitive::Region {
                            points: vec![
                                signex_gerber::Point { x: 0.0, y: 0.0 },
                                signex_gerber::Point { x: 1.0, y: 0.0 },
                                signex_gerber::Point { x: 0.0, y: 1.0 },
                            ],
                            polarity: PrimitivePolarity::Dark,
                        },
                        GerberPrimitive::Flash {
                            position: signex_gerber::Point { x: 2.0, y: 2.0 },
                            aperture: ApertureShape::Polygon {
                                diameter: 1.0,
                                vertices: 6,
                                rotation_degrees: 0.0,
                            },
                            d_code: None,
                            polarity: PrimitivePolarity::Dark,
                        },
                    ],
                );
                let unknown = loaded_layer(
                    "unknown.gbr",
                    signex_gerber::LayerType::UndefinedGerber,
                    vec![GerberPrimitive::Stroke {
                        start: signex_gerber::Point { x: 0.0, y: 0.0 },
                        end: signex_gerber::Point { x: 1.0, y: 0.0 },
                        width: 0.2,
                        d_code: None,
                        polarity: PrimitivePolarity::Dark,
                    }],
                );
                let mut state = GerberViewerState::default();
                state.apply_load_batch(GerberLoadBatch {
                    layers: vec![top, unknown],
                    failures: Vec::new(),
                });

                let export = state.export_native_pcb();

                assert_eq!(export.report.converted_count(), 0);
                assert_eq!(export.report.skipped_count(), 4);
                assert!(
                    export
                        .report
                        .skipped
                        .iter()
                        .any(|item| { item.reason.contains("clear-polarity") })
                );
                assert!(
                    export
                        .report
                        .skipped
                        .iter()
                        .any(|item| { item.reason.contains("filled region") })
                );
                assert!(
                    export
                        .report
                        .skipped
                        .iter()
                        .any(|item| { item.reason.contains("polygon or macro") })
                );
                assert!(
                    export
                        .report
                        .skipped
                        .iter()
                        .any(|item| { item.reason.contains("unsupported layer role") })
                );
            }
        }
    };
}

pub(crate) use gerber_pcb_export_tests;
