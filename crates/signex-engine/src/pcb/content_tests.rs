//! The forward-annotation commands of #1312: add, replace, text, link, nets,
//! and the batch dry run.

use signex_net::board::{BoardNetEdit, PadNetChange, ZoneNetName};
use signex_types::pcb::{Footprint, NetDef, PadNet, PcbBoard, Point, Segment, SymbolLink};
use signex_types::schematic::InstancePath;

use super::tests::{FP_A, FP_B, SEG_1, ZONE_1, board, id, snapshot};
use super::*;

const NEW_FP: &str = "00000000-0000-0000-0000-00000000000c";
const NEW_SEG: &str = "00000000-0000-0000-0000-000000000109";
const PAD_A1: &str = "00000000-0000-0000-0000-0000000000a1";
const GHOST: &str = "00000000-0000-0000-0000-00000000dead";

/// A library footprint in its top-side form: two pads, a silk line, a fab
/// line and Reference/Value texts carrying library placeholders.
fn template(uuid: &str) -> Footprint {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid, "reference": "REF**", "value": "lib value",
        "footprint_id": "SOT-23", "position": {"x": 0.0, "y": 0.0},
        "rotation": 0.0, "layer": "TopCopper",
        "pads": [{
            "uuid": "00000000-0000-0000-0000-0000000000c1", "number": "1",
            "pad_type": "smd", "shape": "round_rect",
            "position": {"x": -0.95, "y": 1.1}, "size": {"x": 0.6, "y": 0.7},
            "drill": null, "layers": ["TopCopper", "TopSolderMask", "TopPaste"], "net": null
        }, {
            "uuid": "00000000-0000-0000-0000-0000000000c2", "number": "2",
            "pad_type": "smd", "shape": "rect",
            "position": {"x": 0.95, "y": 1.1}, "size": {"x": 0.6, "y": 0.7},
            "drill": null, "layers": ["TopCopper", "TopSolderMask", "TopPaste"], "net": null
        }],
        "graphics": [{
            "graphic_type": "line", "layer": "TopSilkscreen", "width": 0.12,
            "start": {"x": -1.5, "y": 0.0}, "end": {"x": 1.5, "y": 0.0},
            "center": null, "mid": null, "position": null, "points": [], "rotation": 0.0
        }, {
            "graphic_type": "line", "layer": "TopFab", "width": 0.1,
            "start": {"x": -1.4, "y": -0.6}, "end": {"x": 1.4, "y": -0.6},
            "center": null, "mid": null, "position": null, "points": [], "rotation": 0.0
        }],
        "properties": [
            {"key": "Reference", "value": "REF**", "position": {"x": 0.0, "y": -2.0},
             "rotation": 0.0, "layer": "TopSilkscreen"},
            {"key": "Value", "value": "lib value", "position": {"x": 0.0, "y": 2.0},
             "rotation": 0.0, "layer": "TopFab"}
        ]
    }))
    .unwrap()
}

fn new_segment() -> Segment {
    serde_json::from_value(serde_json::json!({
        "uuid": NEW_SEG, "start": {"x": 5.0, "y": 0.1}, "end": {"x": 6.0, "y": 0.1},
        "layer": "TopCopper"
    }))
    .unwrap()
}

fn link() -> SymbolLink {
    SymbolLink::new(InstancePath::root(), [id(GHOST), id(NEW_FP)])
}

fn net_edit() -> BoardNetEdit {
    BoardNetEdit {
        nets: vec![
            NetDef {
                number: 1,
                name: "VCC".into(),
            },
            NetDef {
                number: 2,
                name: "GND".into(),
            },
        ],
        pads: vec![PadNetChange {
            footprint: id(FP_A),
            pad: id(PAD_A1),
            net: Some(PadNet {
                number: 2,
                name: "GND".into(),
            }),
            locked: true,
        }],
        zones: vec![ZoneNetName {
            zone: id(ZONE_1),
            name: "GND".into(),
        }],
    }
}

fn new_commands() -> Vec<(&'static str, BoardCommand)> {
    vec![
        (
            "add",
            BoardCommand::AddItems {
                items: vec![
                    BoardItem::Footprint(template(NEW_FP)),
                    BoardItem::Segment(new_segment()),
                ],
            },
        ),
        (
            "replace top",
            BoardCommand::ReplaceFootprint {
                footprint: id(FP_A),
                with: Box::new(template(NEW_FP)),
            },
        ),
        (
            "replace bottom",
            BoardCommand::ReplaceFootprint {
                footprint: id(FP_B),
                with: Box::new(template(NEW_FP)),
            },
        ),
        (
            "text",
            BoardCommand::SetFootprintText {
                footprint: id(FP_A),
                reference: "U7".into(),
                value: "LM358".into(),
            },
        ),
        (
            "link",
            BoardCommand::SetFootprintLink {
                footprint: id(FP_A),
                link: Some(link()),
            },
        ),
        ("nets", BoardCommand::SetNets(net_edit())),
    ]
}

#[test]
fn every_new_command_is_undone_exactly_by_its_inverse() {
    for (name, command) in new_commands() {
        let mut b = board();
        let before = snapshot(&b);
        let applied = command.apply(&mut b).unwrap();
        let after = snapshot(&b);
        assert_ne!(before, after, "{name}: the command changed nothing");
        let redo = applied.inverse.apply(&mut b).unwrap();
        assert_eq!(snapshot(&b), before, "{name}: undo did not restore");
        redo.inverse.apply(&mut b).unwrap();
        assert_eq!(snapshot(&b), after, "{name}: redo did not reproduce");
    }
}

#[test]
fn a_cleared_link_is_put_back_by_its_inverse() {
    let mut b = board();
    BoardCommand::SetFootprintLink {
        footprint: id(FP_A),
        link: Some(link()),
    }
    .apply(&mut b)
    .unwrap();
    let linked = snapshot(&b);
    let cleared = BoardCommand::SetFootprintLink {
        footprint: id(FP_A),
        link: None,
    }
    .apply(&mut b)
    .unwrap();
    assert_eq!(b.footprints[0].schematic_link, None);
    cleared.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), linked);
}

#[test]
fn add_then_delete_round_trips() {
    let mut b = board();
    let original = snapshot(&b);
    let added = BoardCommand::AddItems {
        items: vec![
            BoardItem::Segment(new_segment()),
            BoardItem::Footprint(template(NEW_FP)),
        ],
    }
    .apply(&mut b)
    .unwrap();
    let with_items = snapshot(&b);
    assert_eq!(b.footprints.last().map(|f| f.uuid), Some(id(NEW_FP)));
    assert_eq!(b.segments.last().map(|s| s.uuid), Some(id(NEW_SEG)));
    let BoardCommand::DeleteItems { items } = added.inverse.clone() else {
        panic!("add must invert to a delete");
    };
    assert_eq!(
        items,
        vec![
            BoardItemId::Segment(id(NEW_SEG)),
            BoardItemId::Footprint(id(NEW_FP))
        ]
    );
    let deleted = added.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), original);
    deleted.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), with_items, "the restore puts them back");
}

#[test]
fn replace_keeps_placement_and_identity_and_takes_the_library_geometry() {
    let mut b = board();
    let mut a = b.footprints[0].clone();
    a.locked = true;
    a.schematic_link = Some(link());
    b.footprints[0] = a.clone();
    BoardCommand::ReplaceFootprint {
        footprint: id(FP_A),
        with: Box::new(template(NEW_FP)),
    }
    .apply(&mut b)
    .unwrap();
    let new = &b.footprints[0];
    let lib = template(NEW_FP);
    assert_eq!(
        (new.uuid, new.position, new.rotation, &new.layer, new.locked),
        (a.uuid, a.position, a.rotation, &a.layer, true)
    );
    assert_eq!(new.schematic_link, a.schematic_link);
    assert_eq!((&new.reference, &new.value), (&a.reference, &a.value));
    assert_eq!(new.footprint_id, "SOT-23");
    assert_eq!(new.pads, lib.pads);
    assert_eq!(new.graphics, lib.graphics);
    let texts: Vec<_> = new
        .properties
        .iter()
        .map(|p| (p.key.as_str(), p.value.as_str()))
        .collect();
    assert_eq!(texts, [("Reference", "U1"), ("Value", "")]);
}

#[test]
fn replace_on_the_bottom_flips_the_template_and_keeps_the_rotation() {
    let mut b = board();
    b.footprints[1].rotation = 33.3;
    BoardCommand::ReplaceFootprint {
        footprint: id(FP_B),
        with: Box::new(template(NEW_FP)),
    }
    .apply(&mut b)
    .unwrap();
    let new = &b.footprints[1];
    let mut flipped = template(NEW_FP);
    pcb_geom::flip_footprint(&mut flipped);
    assert_eq!(new.layer, "BottomCopper");
    assert_eq!(new.rotation, 33.3);
    assert_eq!(new.position, board().footprints[1].position);
    assert_eq!(new.pads, flipped.pads);
    assert_eq!(new.graphics, flipped.graphics);
    assert!(new.pads[0].layers.iter().all(|l| l.starts_with("Bottom")));
}

#[test]
fn text_sets_the_fields_and_their_property_texts() {
    let mut b = board();
    b.footprints[0] = {
        let mut fp = template(FP_A);
        fp.uuid = id(FP_A);
        fp
    };
    BoardCommand::SetFootprintText {
        footprint: id(FP_A),
        reference: "Q3".into(),
        value: "BC847".into(),
    }
    .apply(&mut b)
    .unwrap();
    let fp = &b.footprints[0];
    assert_eq!((fp.reference.as_str(), fp.value.as_str()), ("Q3", "BC847"));
    let values: Vec<_> = fp.properties.iter().map(|p| p.value.as_str()).collect();
    assert_eq!(values, ["Q3", "BC847"]);
}

#[test]
fn nets_patch_names_what_it_wrote() {
    let mut b = board();
    let applied = BoardCommand::SetNets(net_edit()).apply(&mut b).unwrap();
    assert_eq!(
        applied.patch,
        BoardPatch {
            kind: BoardPatchKind::NetsChanged,
            items: vec![
                BoardItemId::Footprint(id(FP_A)),
                BoardItemId::Zone(id(ZONE_1))
            ],
        }
    );
    let pad = &b.footprints[0].pads[0];
    assert_eq!(pad.net.as_ref().map(|n| n.name.as_str()), Some("GND"));
    assert!(pad.net_locked);
}

#[test]
fn new_patches_name_the_change() {
    let fp = BoardItemId::Footprint(id(FP_A));
    let kinds = [
        ("add", BoardPatchKind::ItemsAdded),
        ("replace top", BoardPatchKind::FootprintReplaced),
        ("text", BoardPatchKind::FootprintTextSet),
        ("link", BoardPatchKind::FootprintLinkSet),
    ];
    let commands = new_commands();
    for (name, kind) in kinds {
        let command = commands.iter().find(|(n, _)| *n == name).unwrap().1.clone();
        let applied = command.apply(&mut board()).unwrap();
        assert_eq!(applied.patch.kind, kind, "{name}");
        if name != "add" {
            assert_eq!(applied.patch.items, vec![fp], "{name}");
        }
    }
}

#[test]
fn refused_new_commands_leave_the_board_untouched() {
    use BoardCommandError as E;
    let ghost = id(GHOST);
    let seg = BoardItemId::Segment(id(SEG_1));
    let existing_segment = board().segments[0].clone();
    let text_of_a = {
        let mut b = board();
        let applied = BoardCommand::SetFootprintText {
            footprint: id(FP_A),
            reference: "U9".into(),
            value: String::new(),
        }
        .apply(&mut b)
        .unwrap();
        applied.inverse
    };
    let BoardCommand::RestoreFootprintText { text, .. } = text_of_a else {
        panic!("a text edit must invert to a text restore");
    };
    let mut ghost_fp = template(NEW_FP);
    ghost_fp.uuid = ghost;
    let cases: Vec<(BoardCommand, E)> = vec![
        (
            BoardCommand::AddItems {
                items: vec![BoardItem::Segment(existing_segment)],
            },
            E::ItemAlreadyPresent(seg),
        ),
        (
            BoardCommand::AddItems {
                items: vec![
                    BoardItem::Segment(new_segment()),
                    BoardItem::Segment(new_segment()),
                ],
            },
            E::DuplicateItem(BoardItemId::Segment(id(NEW_SEG))),
        ),
        (
            BoardCommand::ReplaceFootprint {
                footprint: ghost,
                with: Box::new(template(NEW_FP)),
            },
            E::FootprintNotFound(ghost),
        ),
        (
            BoardCommand::RestoreFootprint {
                footprint: Box::new(ghost_fp),
            },
            E::FootprintNotFound(ghost),
        ),
        (
            BoardCommand::SetFootprintText {
                footprint: ghost,
                reference: "X".into(),
                value: "Y".into(),
            },
            E::FootprintNotFound(ghost),
        ),
        (
            // FP_B has no properties; the text was taken from FP_A's one.
            BoardCommand::RestoreFootprintText {
                footprint: id(FP_B),
                text,
            },
            E::TextMismatch(id(FP_B)),
        ),
        (
            BoardCommand::SetFootprintLink {
                footprint: ghost,
                link: None,
            },
            E::FootprintNotFound(ghost),
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
fn check_batch_runs_on_a_copy_and_names_the_first_refusal() {
    let b = board();
    let before = snapshot(&b);
    // The move needs the footprint the add brings: commands see the board
    // the earlier ones left.
    let ok = vec![
        BoardCommand::AddItems {
            items: vec![BoardItem::Footprint(template(NEW_FP))],
        },
        BoardCommand::MoveFootprint {
            footprint: id(NEW_FP),
            to: Point { x: 60.0, y: 0.0 },
        },
        BoardCommand::SetNets(net_edit()),
    ];
    assert_eq!(check_batch(&b, &ok), Ok(()));
    let ghost = id(GHOST);
    let refused = vec![
        BoardCommand::SetNets(net_edit()),
        BoardCommand::Flip { footprint: ghost },
        BoardCommand::MoveFootprint {
            footprint: id(FP_A),
            to: Point {
                x: f64::NAN,
                y: 0.0,
            },
        },
    ];
    assert_eq!(
        check_batch(&b, &refused),
        Err(BatchRefusal {
            index: 1,
            error: BoardCommandError::FootprintNotFound(ghost),
        })
    );
    assert_eq!(snapshot(&b), before, "the dry run never writes the board");
}

#[test]
fn an_update_batch_is_one_engine_step_that_undoes_exactly() {
    let mut engine = BoardEngine::new(board(), 1);
    let before = snapshot(engine.board());
    let batch: Vec<BoardCommand> = new_commands().into_iter().map(|(_, c)| c).collect();
    assert_eq!(check_batch(engine.board(), &batch), Ok(()));
    engine.execute_batch(batch).unwrap();
    let after = snapshot(engine.board());
    assert_eq!(engine.undo_len(), 1);
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), before);
    assert!(engine.is_at_save_point());
    engine.redo().unwrap();
    assert_eq!(snapshot(engine.board()), after);
}

/// The template's uuid never reaches the board: the old footprint's does.
#[test]
fn replace_ignores_the_template_uuid() {
    let mut b: PcbBoard = board();
    BoardCommand::ReplaceFootprint {
        footprint: id(FP_A),
        with: Box::new(template(GHOST)),
    }
    .apply(&mut b)
    .unwrap();
    assert_eq!(b.footprints[0].uuid, id(FP_A));
    assert!(b.footprints.iter().all(|f| f.uuid != id(GHOST)));
}
