//! The forward-annotation commands of #1312: add, replace, text, link, nets,
//! and the batch dry run.

use signex_net::board::{BoardNetEdit, PadNetChange, ZoneNetName};
use signex_types::pcb::{Footprint, NetDef, PadNet, PcbBoard, Point, Segment, SymbolLink};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::tests::{FP_A, FP_B, SEG_1, ZONE_1, board, id, snapshot};
use super::*;

const NEW_FP: &str = "00000000-0000-0000-0000-00000000000c";
const NEW_SEG: &str = "00000000-0000-0000-0000-000000000109";
const PAD_A1: &str = "00000000-0000-0000-0000-0000000000a1";
const GHOST: &str = "00000000-0000-0000-0000-00000000dead";

/// A library footprint in its top-side form: two pads, a silk line, a fab
/// line and Reference/Value texts carrying library placeholders. Its pads are
/// `0xc1` and `0xc2`.
fn template(uuid: &str) -> Footprint {
    template_with_pads(uuid, 0xc0)
}

/// [`template`] with pads `pads + 1` and `pads + 2`, so several templates can
/// share one board.
fn template_with_pads(uuid: &str, pads: u128) -> Footprint {
    let mut fp: Footprint = serde_json::from_value(serde_json::json!({
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
    .unwrap();
    for (n, pad) in fp.pads.iter_mut().enumerate() {
        pad.uuid = Uuid::from_u128(pads + 1 + n as u128);
    }
    fp
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
                with: Box::new(template_with_pads(NEW_FP, 0xd0)),
            },
        ),
        (
            "replace bottom",
            BoardCommand::ReplaceFootprint {
                footprint: id(FP_B),
                with: Box::new(template_with_pads(NEW_FP, 0xe0)),
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
    let engine = BoardEngine::new(board(), 1);
    let before = snapshot(engine.board());
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
    assert_eq!(engine.check_batch(&ok), Ok(()));
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
        engine.check_batch(&refused),
        Err(BatchRefusal {
            index: 1,
            error: BoardCommandError::FootprintNotFound(ghost),
        })
    );
    assert_eq!(
        snapshot(engine.board()),
        before,
        "the dry run never writes the board"
    );
    assert_eq!(engine.revision(), 1);
    assert!(!engine.can_undo());
}

#[test]
fn an_update_batch_is_one_engine_step_that_undoes_exactly() {
    let mut engine = BoardEngine::new(board(), 1);
    let before = snapshot(engine.board());
    let batch = update_batch();
    assert_eq!(engine.check_batch(&batch), Ok(()));
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

/// Every new command as one update, the net edit aimed at a pad the replace
/// brought (FP_A's old pad `0xa1` is gone by then).
fn update_batch() -> Vec<BoardCommand> {
    let mut edit = net_edit();
    edit.pads[0].pad = Uuid::from_u128(0xd1);
    new_commands()
        .into_iter()
        .map(|(name, command)| match name {
            "nets" => BoardCommand::SetNets(edit.clone()),
            _ => command,
        })
        .collect()
}

#[test]
fn a_net_edit_aimed_at_a_pad_the_batch_replaced_is_refused_whole() {
    let mut engine = BoardEngine::new(board(), 1);
    let before = snapshot(engine.board());
    let stale = vec![
        BoardCommand::ReplaceFootprint {
            footprint: id(FP_A),
            with: Box::new(template_with_pads(NEW_FP, 0xd0)),
        },
        BoardCommand::SetNets(net_edit()),
    ];
    let refused = BoardCommandError::PadNotFound {
        footprint: id(FP_A),
        pad: id(PAD_A1),
    };
    assert_eq!(
        engine.check_batch(&stale),
        Err(BatchRefusal {
            index: 1,
            error: refused.clone()
        })
    );
    assert_eq!(engine.execute_batch(stale), Err(refused));
    assert_eq!(snapshot(engine.board()), before, "nothing ran");
    assert!(!engine.can_undo());
}

#[test]
fn a_net_edit_naming_something_missing_or_twice_is_refused() {
    use BoardCommandError as E;
    let ghost = id(GHOST);
    let mut ghost_pad = net_edit();
    ghost_pad.pads[0].pad = ghost;
    let mut ghost_footprint = net_edit();
    ghost_footprint.pads[0].footprint = ghost;
    let mut ghost_zone = net_edit();
    ghost_zone.zones[0].zone = ghost;
    let mut twice_pad = net_edit();
    twice_pad.pads.push(twice_pad.pads[0].clone());
    twice_pad.pads[1].locked = false;
    let mut twice_zone = net_edit();
    twice_zone.zones.push(ZoneNetName {
        zone: id(ZONE_1),
        name: "VCC".into(),
    });
    let cases = [
        (
            ghost_pad,
            E::PadNotFound {
                footprint: id(FP_A),
                pad: ghost,
            },
        ),
        (ghost_footprint, E::FootprintNotFound(ghost)),
        (ghost_zone, E::ItemNotFound(BoardItemId::Zone(ghost))),
        (
            twice_pad,
            E::DuplicatePadChange {
                footprint: id(FP_A),
                pad: id(PAD_A1),
            },
        ),
        (twice_zone, E::DuplicateItem(BoardItemId::Zone(id(ZONE_1)))),
    ];
    for (edit, expected) in cases {
        let mut b = board();
        let before = snapshot(&b);
        assert_eq!(
            BoardCommand::SetNets(edit.clone()).apply(&mut b),
            Err(expected),
            "{edit:?}"
        );
        assert_eq!(snapshot(&b), before);
    }
}

#[test]
fn a_locked_pad_keeps_its_net_and_lock_through_a_replace_and_the_next_sync() {
    use signex_net::board::{SymbolPadKey, assign_pad_nets};
    use signex_types::net::{Net, NetId, Netlist, Terminal};
    let symbol = id(GHOST);
    let mut b = board();
    let fp = &mut b.footprints[0];
    fp.schematic_link = Some(SymbolLink::new(InstancePath::root(), [symbol]));
    fp.pads[0].net = Some(PadNet {
        number: 7,
        name: "HAND".into(),
    });
    fp.pads[0].net_locked = true;
    fp.pads[1].net = Some(PadNet {
        number: 8,
        name: "OLD".into(),
    });
    let mut lib = template_with_pads(NEW_FP, 0xd0);
    lib.pads.push(lib.pads[1].clone());
    lib.pads[2].uuid = Uuid::from_u128(0xd3);
    lib.pads[2].number = "3".into();
    BoardCommand::ReplaceFootprint {
        footprint: id(FP_A),
        with: Box::new(lib),
    }
    .apply(&mut b)
    .unwrap();
    let pads = &b.footprints[0].pads;
    assert_eq!(pads[0].net.as_ref().map(|n| n.name.as_str()), Some("HAND"));
    assert!(pads[0].net_locked, "the lock is carried by pad number");
    assert_eq!(pads[1].net.as_ref().map(|n| n.name.as_str()), Some("OLD"));
    assert!(!pads[1].net_locked);
    assert_eq!(pads[2].net, None, "a number the old footprint lacks");

    // The schematic puts pin 1 on SIG; the sync leaves the locked pad alone.
    let netlist = Netlist {
        nets: vec![Net {
            id: NetId(1),
            name: "SIG".into(),
            class: None,
            wires: Vec::new(),
            junctions: Vec::new(),
            terminals: vec![Terminal {
                symbol,
                reference: "U1".into(),
                pin: "1".into(),
                occurrence: InstancePath::root(),
            }],
        }],
    };
    let sync = assign_pad_nets(&netlist, &b, &SymbolPadKey::for_board(&b));
    BoardCommand::SetNets(sync.edit).apply(&mut b).unwrap();
    let pad = &b.footprints[0].pads[0];
    assert_eq!(pad.net.as_ref().map(|n| n.name.as_str()), Some("HAND"));
    assert!(pad.net_locked);
}

#[test]
fn pad_uuids_stay_unique_board_wide() {
    use BoardCommandError as E;
    let a1 = id(PAD_A1);
    let mut clash = template_with_pads(NEW_FP, 0xd0);
    clash.pads[0].uuid = a1;
    let mut twice = template_with_pads(NEW_FP, 0xd0);
    twice.pads[1].uuid = twice.pads[0].uuid;
    let mut second = template_with_pads(GHOST, 0xd0);
    second.uuid = Uuid::from_u128(0xbeef);
    let cases = [
        (
            BoardCommand::AddItems {
                items: vec![BoardItem::Footprint(clash.clone())],
            },
            E::PadAlreadyPresent(a1),
        ),
        (
            BoardCommand::AddItems {
                items: vec![
                    BoardItem::Footprint(template_with_pads(NEW_FP, 0xd0)),
                    BoardItem::Footprint(second),
                ],
            },
            E::PadAlreadyPresent(Uuid::from_u128(0xd1)),
        ),
        (
            BoardCommand::ReplaceFootprint {
                footprint: id(FP_B),
                with: Box::new(clash.clone()),
            },
            E::PadAlreadyPresent(a1),
        ),
        (
            BoardCommand::ReplaceFootprint {
                footprint: id(FP_B),
                with: Box::new(twice),
            },
            E::PadAlreadyPresent(Uuid::from_u128(0xd1)),
        ),
    ];
    for (command, expected) in cases {
        let mut b = board();
        let before = snapshot(&b);
        assert_eq!(command.clone().apply(&mut b), Err(expected), "{command:?}");
        assert_eq!(snapshot(&b), before);
    }
    // A footprint may keep its own pad uuids through a replace.
    let mut b = board();
    BoardCommand::ReplaceFootprint {
        footprint: id(FP_A),
        with: Box::new(clash),
    }
    .apply(&mut b)
    .unwrap();
}

#[test]
fn a_new_item_off_the_number_line_and_a_bottom_template_are_refused() {
    use BoardCommandError as E;
    let mut at_nan = template(NEW_FP);
    at_nan.position.y = f64::NAN;
    let mut turned_inf = template(NEW_FP);
    turned_inf.rotation = f64::INFINITY;
    let mut segment = new_segment();
    segment.end.x = f64::NEG_INFINITY;
    let mut bottom = template(NEW_FP);
    bottom.layer = "BottomCopper".into();
    let cases = [
        (
            BoardCommand::AddItems {
                items: vec![BoardItem::Footprint(at_nan)],
            },
            E::NonFinite { field: "position" },
        ),
        (
            BoardCommand::AddItems {
                items: vec![BoardItem::Footprint(turned_inf)],
            },
            E::NonFinite { field: "rotation" },
        ),
        (
            BoardCommand::AddItems {
                items: vec![BoardItem::Segment(segment)],
            },
            E::NonFinite { field: "position" },
        ),
        (
            BoardCommand::ReplaceFootprint {
                footprint: id(FP_A),
                with: Box::new(bottom),
            },
            E::TemplateNotTopSide(id(NEW_FP)),
        ),
    ];
    for (command, expected) in cases {
        let mut b = board();
        let before = snapshot(&b);
        assert_eq!(command.clone().apply(&mut b), Err(expected), "{command:?}");
        assert_eq!(snapshot(&b), before);
    }
}
