use signex_types::part::FootprintResolveError;
use signex_types::pcb::{self, PadType};

use super::*;
use crate::primitive::footprint::{ChamferedCorners, FpCutout, Polygon};

fn pad(number: &str, kind: PadKind, shape: PadShape, layers: &[&str]) -> Pad {
    Pad {
        number: number.into(),
        kind,
        shape,
        size: [1.2, 0.7],
        position: [-0.75, 0.1],
        rotation: 30.0,
        layers: layers.iter().map(|l| LayerId::new(*l)).collect(),
        ..Pad::default()
    }
}

fn library(pads: Vec<Pad>) -> Footprint {
    let mut fp = Footprint::empty("SOT-23");
    fp.pads = pads;
    fp
}

fn convert_one(pad: Pad) -> Result<pcb::Pad, FootprintResolveError> {
    board_footprint(&library(vec![pad])).map(|fp| fp.pads.into_iter().next().unwrap())
}

#[test]
fn every_pad_kind_maps_to_its_board_type() {
    let cases = [
        (PadKind::Smd, PadType::Smd),
        (PadKind::Tht, PadType::Thru),
        (PadKind::NptHole, PadType::NpThru),
        (PadKind::ConnectorPad, PadType::Connect),
        (PadKind::Castellated, PadType::Thru),
        (PadKind::Fiducial, PadType::Smd),
    ];
    for (kind, expected) in cases {
        let board = convert_one(pad("1", kind, PadShape::Rect, &["F.Cu"])).unwrap();
        assert_eq!(board.pad_type, expected, "{kind:?}");
    }
}

#[test]
fn every_held_shape_maps_and_keeps_its_corner_ratio() {
    let cases = [
        (PadShape::Round, pcb::PadShape::Circle, 0.0),
        (PadShape::Rect, pcb::PadShape::Rect, 0.0),
        (PadShape::Oval, pcb::PadShape::Oval, 0.0),
        (
            PadShape::RoundRect { radius_ratio: 0.25 },
            pcb::PadShape::RoundRect,
            0.25,
        ),
    ];
    for (shape, expected, ratio) in cases {
        let board = convert_one(pad("1", PadKind::Smd, shape.clone(), &["F.Cu"])).unwrap();
        assert_eq!(
            (board.shape, board.roundrect_ratio),
            (expected, ratio),
            "{shape:?}"
        );
    }
}

#[test]
fn shapes_the_board_cannot_hold_are_refused() {
    let cases = [
        (
            PadShape::Chamfered {
                chamfer_ratio: 0.2,
                corners: ChamferedCorners::all(),
            },
            "pad shape",
            "chamfered",
        ),
        (
            PadShape::Custom(Polygon::new(vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])),
            "pad shape",
            "custom",
        ),
        (
            PadShape::RoundRect { radius_ratio: 0.7 },
            "pad corner ratio",
            "0.7",
        ),
    ];
    for (shape, what, value) in cases {
        assert_eq!(
            convert_one(pad("1", PadKind::Smd, shape, &["F.Cu"])),
            Err(FootprintResolveError::Unconvertible {
                what,
                value: value.into()
            })
        );
    }
}

#[test]
fn size_position_rotation_and_drill_are_carried_exactly() {
    let mut tht = pad("2", PadKind::Tht, PadShape::Round, &["*.Cu", "*.Mask"]);
    tht.drill = Some(Drill {
        diameter: 0.8,
        slot_length: None,
    });
    let board = convert_one(tht).unwrap();
    assert_eq!(board.number, "2");
    assert_eq!(board.size, Point { x: 1.2, y: 0.7 });
    assert_eq!(board.position, Point { x: -0.75, y: 0.1 });
    assert_eq!(board.rotation, 30.0);
    assert_eq!(board.drill.map(|d| d.diameter), Some(0.8));
    assert_eq!(
        board.layers,
        [
            "TopCopper",
            "BottomCopper",
            "TopSolderMask",
            "BottomSolderMask"
        ]
    );
    assert_eq!(board.net, None);
}

#[test]
fn a_slot_and_a_copper_offset_are_refused() {
    let mut slot = pad("1", PadKind::Tht, PadShape::Oval, &["*.Cu"]);
    slot.drill = Some(Drill {
        diameter: 0.8,
        slot_length: Some(2.0),
    });
    assert!(matches!(
        convert_one(slot),
        Err(FootprintResolveError::Unconvertible {
            what: "slotted drill of length",
            ..
        })
    ));
    let mut offset = pad("1", PadKind::Smd, PadShape::Rect, &["F.Cu"]);
    offset.copper_offset_x_mm = Some(0.1);
    assert!(matches!(
        convert_one(offset),
        Err(FootprintResolveError::Unconvertible {
            what: "pad copper offset",
            ..
        })
    ));
}

#[test]
fn layer_tokens_read_through_the_board_layer_model() {
    let board = convert_one(pad(
        "1",
        PadKind::Smd,
        PadShape::Rect,
        &["F.Cu", "TopCopper", "F.Mask", "F.Paste", "In2.Cu", "B.Cu"],
    ))
    .unwrap();
    assert_eq!(
        board.layers,
        [
            "TopCopper",
            "TopSolderMask",
            "TopPaste",
            "Inner2Copper",
            "BottomCopper"
        ],
        "each layer once, board tokens kept"
    );
}

#[test]
fn an_unknown_layer_token_is_refused_never_guessed() {
    for token in ["F.Copper", "In02.Cu", "In.Cu", "Dwgs.User", "topcopper", ""] {
        assert_eq!(
            convert_one(pad("1", PadKind::Smd, PadShape::Rect, &[token])),
            Err(FootprintResolveError::Unconvertible {
                what: "footprint layer",
                value: token.into()
            }),
            "{token:?}"
        );
    }
}

#[test]
fn silk_fab_and_courtyard_land_on_their_board_layers() {
    let mut fp = library(Vec::new());
    let line = FpGraphic::drawn(
        FpGraphicKind::Line {
            from: [-1.0, 0.5],
            to: [1.0, 0.5],
        },
        0.12,
        false,
    );
    let filled = FpGraphic::drawn(
        FpGraphicKind::Polygon {
            vertices: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        },
        0.0,
        true,
    );
    fp.silk_f = vec![line.clone()];
    fp.silk_b = vec![line.clone()];
    fp.fab_f = vec![filled];
    fp.fab_b = vec![FpGraphic::drawn(
        FpGraphicKind::Circle {
            center: [0.0, 0.0],
            radius: 0.4,
        },
        0.1,
        false,
    )];
    fp.courtyard = Polygon::new(vec![[-2.0, -2.0], [2.0, -2.0], [2.0, 2.0]]);
    let board = board_footprint(&fp).unwrap();
    let layers: Vec<_> = board
        .graphics
        .iter()
        .map(|g| (g.graphic_type.as_str(), g.layer.as_str()))
        .collect();
    assert_eq!(
        layers,
        [
            ("line", "TopSilkscreen"),
            ("line", "BottomSilkscreen"),
            ("poly", "TopFab"),
            ("circle", "BottomFab"),
            ("poly", "TopCourtyard"),
        ]
    );
    let silk = &board.graphics[0];
    assert_eq!(silk.start, Some(Point { x: -1.0, y: 0.5 }));
    assert_eq!(silk.end, Some(Point { x: 1.0, y: 0.5 }));
    assert_eq!(silk.width, 0.12);
    assert_eq!(board.graphics[2].fill, "solid");
    assert_eq!(board.graphics[3].radius, 0.4);
    assert_eq!(board.graphics[4].points.len(), 3);
}

#[test]
fn an_arc_keeps_its_sweep_through_the_middle_point() {
    let mut fp = library(Vec::new());
    fp.silk_f = vec![FpGraphic::drawn(
        FpGraphicKind::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_deg: 300.0,
            end_deg: 60.0,
        },
        0.1,
        false,
    )];
    let arc = &board_footprint(&fp).unwrap().graphics[0];
    let mid = arc.mid.unwrap();
    // 300 -> 60 is the 120 degree sweep through 0, so the middle is at +x.
    assert!(
        (mid.x - 1.0).abs() < 1e-12 && mid.y.abs() < 1e-12,
        "{mid:?}"
    );
}

#[test]
fn a_framed_text_and_a_feature_the_board_cannot_hold_are_refused() {
    let mut fp = library(Vec::new());
    fp.silk_f = vec![FpGraphic::drawn(
        FpGraphicKind::Text {
            position: [0.0, 0.0],
            content: "X".into(),
            size: 1.0,
            frame: Some((2.0, 1.0)),
        },
        0.1,
        false,
    )];
    assert!(matches!(
        board_footprint(&fp),
        Err(FootprintResolveError::Unconvertible {
            what: "framed footprint text",
            ..
        })
    ));
    let mut fp = library(Vec::new());
    fp.cutouts = vec![FpCutout {
        boundary: Polygon::default(),
        edge_radius_mm: 0.0,
        through: true,
    }];
    assert_eq!(
        board_footprint(&fp),
        Err(FootprintResolveError::Unconvertible {
            what: "footprint cutouts",
            value: "1".into()
        })
    );
}

#[test]
fn the_board_footprint_is_top_side_at_the_origin_with_empty_texts() {
    let lib = library(vec![pad("1", PadKind::Smd, PadShape::Rect, &["F.Cu"])]);
    let board = board_footprint(&lib).unwrap();
    assert_eq!(board.footprint_id, "SOT-23");
    assert_eq!(board.layer, "TopCopper");
    assert_eq!(
        (board.position, board.rotation),
        (Point { x: 0.0, y: 0.0 }, 0.0)
    );
    assert_eq!((board.schematic_link, board.library_ref), (None, None));
    let texts: Vec<_> = board
        .properties
        .iter()
        .map(|p| (p.key.as_str(), p.value.as_str(), p.layer.as_deref()))
        .collect();
    assert_eq!(
        texts,
        [
            ("Reference", "", Some("TopSilkscreen")),
            ("Value", "", Some("TopFab"))
        ]
    );
    assert_ne!(board.uuid, lib.uuid, "each placement gets its own uuid");
    let again = board_footprint(&lib).unwrap();
    assert_ne!(again.pads[0].uuid, board.pads[0].uuid);
}

#[test]
fn pad_settings_the_board_cannot_hold_are_refused() {
    use signex_sketch::attr::{ElectricalType, PadFeature, TestpointFlags};
    type Set = fn(&mut Pad);
    let cases: [(&str, Set); 9] = [
        ("pad tented on top", |p| p.mask_tented_top = true),
        ("pad tented on bottom", |p| p.mask_tented_bottom = true),
        ("pad without paste on top", |p| p.paste_enabled_top = false),
        ("pad without paste on bottom", |p| {
            p.paste_enabled_bottom = false
        }),
        ("pad without thermal relief", |p| p.thermal_relief = false),
        ("pad top feature", |p| {
            p.feature_top = PadFeature::Counterbore
        }),
        ("pad bottom feature", |p| {
            p.feature_bottom = PadFeature::Countersink
        }),
        ("pad test point", |p| {
            p.testpoint = TestpointFlags {
                top_fab: true,
                ..TestpointFlags::default()
            }
        }),
        ("pad electrical type", |p| {
            p.electrical_type = ElectricalType::Source
        }),
    ];
    for (what, set) in cases {
        let mut p = pad("7", PadKind::Smd, PadShape::Rect, &["F.Cu"]);
        set(&mut p);
        assert_eq!(
            convert_one(p),
            Err(FootprintResolveError::Unconvertible {
                what,
                value: "7".into()
            })
        );
    }
}

#[test]
fn margins_are_not_carried_and_do_not_refuse() {
    let mut p = pad("1", PadKind::Smd, PadShape::Rect, &["F.Cu"]);
    p.solder_mask_margin = Some(0.05);
    p.paste_margin = Some(-0.02);
    p.mask_margin_top = Some(0.1);
    p.paste_margin_bottom = Some(0.1);
    assert!(convert_one(p).is_ok());
}

#[test]
fn a_3d_body_a_step_model_and_an_anchor_are_refused() {
    use crate::primitive::footprint::StepAttachment;
    let mut body = library(Vec::new());
    body.body_3d.height_mm = 2.5;
    let mut step = library(Vec::new());
    step.step_attachment = Some(StepAttachment {
        content_hash: "ab".into(),
        filename: "SOT-23.step".into(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    });
    let mut anchor = library(Vec::new());
    anchor.anchor = [0.5, 0.0];
    let cases = [
        (body, "footprint 3D body", "set"),
        (step, "footprint STEP model", "SOT-23.step"),
        (anchor, "footprint anchor", "[0.5, 0.0]"),
    ];
    for (fp, what, value) in cases {
        assert_eq!(
            board_footprint(&fp),
            Err(FootprintResolveError::Unconvertible {
                what,
                value: value.into()
            })
        );
    }
}
