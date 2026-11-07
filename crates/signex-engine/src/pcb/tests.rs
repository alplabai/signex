use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::*;

const FP_A: &str = "00000000-0000-0000-0000-00000000000a";
const FP_B: &str = "00000000-0000-0000-0000-00000000000b";
const SEG_1: &str = "00000000-0000-0000-0000-000000000101";
const SEG_2: &str = "00000000-0000-0000-0000-000000000102";
const SEG_3: &str = "00000000-0000-0000-0000-000000000103";
const VIA_1: &str = "00000000-0000-0000-0000-000000000201";
const ZONE_1: &str = "00000000-0000-0000-0000-000000000301";
const TEXT_1: &str = "00000000-0000-0000-0000-000000000401";

fn id(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap()
}

/// Values chosen to be inexact in binary (0.1, 1/3, 1e-7) so a delta-based
/// inverse would leave a different bit pattern behind.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": "00000000-0000-0000-0000-000000000001",
        "outline": [{"x": 0.0, "y": 0.0}, {"x": 50.1, "y": 0.0}, {"x": 50.1, "y": 30.3}],
        "footprints": [
            {
                "uuid": FP_A, "reference": "U1", "position": {"x": 10.1, "y": 20.2},
                "rotation": 33.333333333333336, "layer": "F.Cu",
                "pads": [{
                    "uuid": "00000000-0000-0000-0000-0000000000a1", "number": "1",
                    "pad_type": "smd", "shape": "rect",
                    "position": {"x": -1.27, "y": 0.1}, "size": {"x": 0.6, "y": 1.1},
                    "drill": null, "layers": ["F.Cu", "F.Mask", "F.Paste"], "net": null
                }, {
                    "uuid": "00000000-0000-0000-0000-0000000000a2", "number": "2",
                    "pad_type": "thru", "shape": "circle",
                    "position": {"x": 0.0, "y": -0.0000001}, "size": {"x": 1.0, "y": 1.0},
                    "drill": {"diameter": 0.5}, "layers": ["*.Cu", "*.Mask"], "net": null
                }],
                "graphics": [{
                    "graphic_type": "line", "layer": "F.SilkS", "width": 0.15,
                    "start": {"x": -2.5, "y": 1.0}, "end": {"x": 2.5, "y": 1.0},
                    "center": null, "mid": null, "position": null,
                    "points": [{"x": 0.3, "y": 0.7}], "rotation": 12.5
                }],
                "properties": [{
                    "key": "Reference", "value": "U1", "position": {"x": 0.7, "y": -2.0},
                    "rotation": 0.0, "layer": "F.SilkS"
                }]
            },
            {
                "uuid": FP_B, "reference": "R1", "position": {"x": 0.30000000000000004, "y": 7.0},
                "rotation": 0.0, "layer": "B.Cu"
            }
        ],
        "segments": [
            {"uuid": SEG_1, "start": {"x": 0.0, "y": 0.0}, "end": {"x": 1.0, "y": 0.0}, "layer": "F.Cu"},
            {"uuid": SEG_2, "start": {"x": 1.0, "y": 0.0}, "end": {"x": 2.0, "y": 0.0}, "layer": "F.Cu"},
            {"uuid": SEG_3, "start": {"x": 2.0, "y": 0.0}, "end": {"x": 3.0, "y": 0.0}, "layer": "B.Cu"}
        ],
        "vias": [{"uuid": VIA_1, "position": {"x": 3.0, "y": 0.0}, "layers": ["F.Cu", "B.Cu"]}],
        "zones": [{"uuid": ZONE_1, "layer": "B.Cu", "outline": [{"x": 0.0, "y": 0.0}]}],
        "texts": [{"uuid": TEXT_1, "text": "rev A", "position": {"x": 4.0, "y": 4.0}}]
    }))
    .unwrap()
}

/// Exact snapshot: the JSON text writes each f64 in its shortest round-trip
/// form, so two boards give the same text only if every bit matches
/// (0.0 and -0.0 included).
fn snapshot(board: &PcbBoard) -> String {
    serde_json::to_string(board).unwrap()
}

fn commands() -> Vec<(&'static str, BoardCommand)> {
    use BoardItemId::*;
    vec![
        (
            "move",
            BoardCommand::MoveFootprint {
                footprint: id(FP_A),
                to: Point {
                    x: 0.1 + 0.2,
                    y: -1.0 / 3.0,
                },
            },
        ),
        (
            "rotate",
            BoardCommand::Rotate {
                footprint: id(FP_A),
                degrees: 90.0 + 0.1,
            },
        ),
        (
            "flip top",
            BoardCommand::Flip {
                footprint: id(FP_A),
            },
        ),
        (
            "flip bottom",
            BoardCommand::Flip {
                footprint: id(FP_B),
            },
        ),
        (
            "outline",
            BoardCommand::SetBoardOutline {
                outline: vec![Point { x: 1.1, y: 2.2 }, Point { x: 3.3, y: 4.4 }],
            },
        ),
        (
            "outline cleared",
            BoardCommand::SetBoardOutline {
                outline: Vec::new(),
            },
        ),
        (
            "delete one",
            BoardCommand::DeleteItems {
                items: vec![Segment(id(SEG_2))],
            },
        ),
        (
            "delete mixed, out of order",
            BoardCommand::DeleteItems {
                items: vec![
                    Segment(id(SEG_3)),
                    Text(id(TEXT_1)),
                    Footprint(id(FP_A)),
                    Segment(id(SEG_1)),
                    Via(id(VIA_1)),
                    Zone(id(ZONE_1)),
                ],
            },
        ),
        (
            "delete every footprint",
            BoardCommand::DeleteItems {
                items: vec![Footprint(id(FP_B)), Footprint(id(FP_A))],
            },
        ),
    ]
}

#[test]
fn every_command_is_undone_exactly_by_its_inverse() {
    for (name, command) in commands() {
        let mut b = board();
        let before = snapshot(&b);
        let applied = command.apply(&mut b).unwrap();
        let after = snapshot(&b);
        assert_ne!(before, after, "{name}: the command changed nothing");

        let redo = applied.inverse.apply(&mut b).unwrap();
        assert_eq!(
            snapshot(&b),
            before,
            "{name}: undo did not restore the board"
        );

        redo.inverse.apply(&mut b).unwrap();
        assert_eq!(
            snapshot(&b),
            after,
            "{name}: redo did not reproduce the edit"
        );
    }
}

#[test]
fn patches_name_the_change() {
    let mut b = board();
    let fp = BoardItemId::Footprint(id(FP_A));
    let cases = [
        (
            BoardCommand::Flip {
                footprint: id(FP_A),
            },
            BoardPatchKind::FootprintFlipped,
            vec![fp],
        ),
        (
            BoardCommand::SetBoardOutline {
                outline: Vec::new(),
            },
            BoardPatchKind::OutlineReplaced,
            Vec::new(),
        ),
        (
            BoardCommand::DeleteItems { items: vec![fp] },
            BoardPatchKind::ItemsDeleted,
            vec![fp],
        ),
    ];
    for (command, kind, items) in cases {
        let applied = command.apply(&mut b).unwrap();
        assert_eq!(applied.patch, BoardPatch { kind, items });
    }
}

/// Deterministic xorshift, so a failure replays.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn mm(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 * 400.0 - 200.0
    }
}

fn live_ids(b: &PcbBoard) -> Vec<BoardItemId> {
    let mut ids: Vec<_> = b
        .footprints
        .iter()
        .map(|x| BoardItemId::Footprint(x.uuid))
        .collect();
    ids.extend(b.segments.iter().map(|x| BoardItemId::Segment(x.uuid)));
    ids.extend(b.vias.iter().map(|x| BoardItemId::Via(x.uuid)));
    ids.extend(b.zones.iter().map(|x| BoardItemId::Zone(x.uuid)));
    ids.extend(b.texts.iter().map(|x| BoardItemId::Text(x.uuid)));
    ids
}

fn random_command(rng: &mut Rng, b: &PcbBoard) -> Option<BoardCommand> {
    let footprint =
        (!b.footprints.is_empty()).then(|| b.footprints[rng.below(b.footprints.len())].uuid);
    match rng.below(5) {
        0 => footprint.map(|footprint| BoardCommand::MoveFootprint {
            footprint,
            to: Point {
                x: rng.mm(),
                y: rng.mm(),
            },
        }),
        1 => footprint.map(|footprint| BoardCommand::Rotate {
            footprint,
            degrees: rng.mm(),
        }),
        2 => footprint.map(|footprint| BoardCommand::Flip { footprint }),
        3 => {
            let n = rng.below(6);
            let outline = (0..n)
                .map(|_| Point {
                    x: rng.mm(),
                    y: rng.mm(),
                })
                .collect();
            Some(BoardCommand::SetBoardOutline { outline })
        }
        _ => {
            let live = live_ids(b);
            if live.is_empty() {
                return None;
            }
            let items = live
                .into_iter()
                .filter(|_| rng.below(3) == 0)
                .collect::<Vec<_>>();
            (!items.is_empty()).then_some(BoardCommand::DeleteItems { items })
        }
    }
}

#[test]
fn random_edit_sequences_unwind_to_the_original_board() {
    for seed in 1..=64u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut b = board();
        let original = snapshot(&b);
        let mut states = vec![original.clone()];
        let mut undo = Vec::new();
        for _ in 0..40 {
            let Some(command) = random_command(&mut rng, &b) else {
                continue;
            };
            undo.push(command.apply(&mut b).unwrap().inverse);
            states.push(snapshot(&b));
        }
        while let Some(inverse) = undo.pop() {
            states.pop();
            inverse.apply(&mut b).unwrap();
            assert_eq!(&snapshot(&b), states.last().unwrap(), "seed {seed}");
        }
        assert_eq!(snapshot(&b), original, "seed {seed}");
    }
}

#[test]
fn flip_mirrors_local_geometry_and_swaps_sides() {
    let mut b = board();
    BoardCommand::Flip {
        footprint: id(FP_A),
    }
    .apply(&mut b)
    .unwrap();
    let fp = &b.footprints[0];
    assert_eq!(fp.layer, "B.Cu");
    assert_eq!(fp.rotation, -33.333333333333336);
    assert_eq!(fp.position, Point { x: 10.1, y: 20.2 });
    assert_eq!(fp.pads[0].position, Point { x: 1.27, y: 0.1 });
    assert_eq!(fp.pads[0].layers, ["B.Cu", "B.Mask", "B.Paste"]);
    assert_eq!(fp.pads[1].layers, ["*.Cu", "*.Mask"]);
    assert!(fp.pads[1].position.x.is_sign_positive(), "0 stays +0");
    let g = &fp.graphics[0];
    assert_eq!(g.layer, "B.SilkS");
    assert_eq!(g.start, Some(Point { x: 2.5, y: 1.0 }));
    assert_eq!(g.points, [Point { x: -0.3, y: 0.7 }]);
    assert_eq!(g.rotation, -12.5);
    assert_eq!(fp.properties[0].layer.as_deref(), Some("B.SilkS"));
    assert_eq!(fp.properties[0].position, Some(Point { x: -0.7, y: -2.0 }));
}

#[test]
fn delete_inverse_keeps_list_order() {
    let mut b = board();
    let applied = BoardCommand::DeleteItems {
        items: vec![
            BoardItemId::Segment(id(SEG_3)),
            BoardItemId::Segment(id(SEG_1)),
        ],
    }
    .apply(&mut b)
    .unwrap();
    assert_eq!(b.segments.len(), 1);
    let BoardCommand::RestoreItems { items } = &applied.inverse else {
        panic!("delete must invert to a restore");
    };
    assert_eq!(items.iter().map(|r| r.index).collect::<Vec<_>>(), [0, 2]);
    applied.inverse.apply(&mut b).unwrap();
    let order: Vec<_> = b.segments.iter().map(|s| s.uuid).collect();
    assert_eq!(order, [id(SEG_1), id(SEG_2), id(SEG_3)]);
}

#[test]
fn refused_commands_leave_the_board_untouched() {
    use BoardCommandError as E;
    let ghost = id("00000000-0000-0000-0000-00000000dead");
    let seg = BoardItemId::Segment(id(SEG_1));
    let removed_seg = {
        let mut b = board();
        let BoardCommand::RestoreItems { items } = BoardCommand::DeleteItems { items: vec![seg] }
            .apply(&mut b)
            .unwrap()
            .inverse
        else {
            panic!("delete must invert to a restore");
        };
        items.into_iter().next().unwrap()
    };
    let nan = Point {
        x: f64::NAN,
        y: 0.0,
    };
    let cases: Vec<(BoardCommand, E)> = vec![
        (
            BoardCommand::MoveFootprint {
                footprint: ghost,
                to: Point { x: 0.0, y: 0.0 },
            },
            E::FootprintNotFound(ghost),
        ),
        (
            BoardCommand::MoveFootprint {
                footprint: id(FP_A),
                to: nan,
            },
            E::NonFinite { field: "position" },
        ),
        (
            BoardCommand::Rotate {
                footprint: id(FP_A),
                degrees: f64::INFINITY,
            },
            E::NonFinite { field: "rotation" },
        ),
        (
            BoardCommand::Flip { footprint: ghost },
            E::FootprintNotFound(ghost),
        ),
        (
            BoardCommand::SetBoardOutline {
                outline: vec![Point { x: 1.0, y: 1.0 }, nan],
            },
            E::NonFinite { field: "outline" },
        ),
        (
            BoardCommand::DeleteItems {
                items: vec![BoardItemId::Via(id(VIA_1)), seg, seg],
            },
            E::DuplicateItem(seg),
        ),
        (
            BoardCommand::DeleteItems {
                items: vec![seg, BoardItemId::Zone(ghost)],
            },
            E::ItemNotFound(BoardItemId::Zone(ghost)),
        ),
        (
            BoardCommand::RestoreItems {
                items: vec![removed_seg.clone()],
            },
            E::ItemAlreadyPresent(seg),
        ),
    ];
    for (command, expected) in cases {
        let mut b = board();
        let before = snapshot(&b);
        assert_eq!(command.clone().apply(&mut b), Err(expected), "{command:?}");
        assert_eq!(snapshot(&b), before, "{command:?} wrote before refusing");
    }
}

#[test]
fn restore_refuses_a_gap_or_a_shared_index() {
    let mut b = board();
    let BoardCommand::RestoreItems { items } = BoardCommand::DeleteItems {
        items: vec![
            BoardItemId::Segment(id(SEG_1)),
            BoardItemId::Segment(id(SEG_2)),
        ],
    }
    .apply(&mut b)
    .unwrap()
    .inverse
    else {
        panic!("delete must invert to a restore");
    };
    let before = snapshot(&b);

    let mut gap = items.clone();
    gap[1].index = 3;
    assert_eq!(
        BoardCommand::RestoreItems { items: gap }.apply(&mut b),
        Err(BoardCommandError::RestoreIndexOutOfRange {
            id: BoardItemId::Segment(id(SEG_2)),
            index: 3,
            len: 2,
        })
    );
    let mut shared = items;
    shared[1].index = 0;
    assert_eq!(
        BoardCommand::RestoreItems { items: shared }.apply(&mut b),
        Err(BoardCommandError::DuplicateRestoreIndex { index: 0 })
    );
    assert_eq!(snapshot(&b), before);
}

#[test]
fn layer_names_mirror_by_side_prefix() {
    for (name, mirrored) in [
        ("F.Cu", "B.Cu"),
        ("B.SilkS", "F.SilkS"),
        ("F.Courtyard", "B.Courtyard"),
        ("In2.Cu", "In2.Cu"),
        ("*.Cu", "*.Cu"),
        ("Mechanical 1", "Mechanical 1"),
        ("", ""),
    ] {
        assert_eq!(mirror_layer_name(name), mirrored);
        assert_eq!(mirror_layer_name(mirrored), name);
    }
}
