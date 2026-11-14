//! `BoardCommand::ReplaceItems` (#2355): in-place edits of tracks and vias,
//! undone bit for bit, refused whole when a value cannot be held.

use signex_types::pcb::{NetDef, PcbBoard, Point, Segment, Via};

use super::tests::{FP_A, SEG_1, SEG_2, VIA_1, board, id, snapshot};
use super::*;

fn with_nets() -> PcbBoard {
    let mut b = board();
    b.nets = vec![NetDef {
        number: 7,
        name: "GND".into(),
    }];
    b.vias[0].diameter = 0.6;
    b.vias[0].drill = 0.3;
    b
}

fn segment(b: &PcbBoard, uuid: &str) -> Segment {
    b.segments.iter().find(|s| s.uuid == id(uuid)).unwrap().clone()
}

fn via(b: &PcbBoard) -> Via {
    b.vias[0].clone()
}

fn replace(items: Vec<BoardItem>) -> BoardCommand {
    BoardCommand::ReplaceItems { items }
}

#[test]
fn replacing_tracks_and_a_via_keeps_their_place_and_undoes_exactly() {
    let mut b = with_nets();
    let before = snapshot(&b);
    let mut s1 = segment(&b, SEG_1);
    s1.width = 0.1 + 0.2;
    s1.start = Point {
        x: -1.0 / 3.0,
        y: 0.7,
    };
    s1.layer = "BottomCopper".into();
    s1.net = 7;
    let mut v = via(&b);
    v.diameter = 0.8;
    v.drill = 0.4;
    v.net = 7;
    let applied = replace(vec![BoardItem::Segment(s1.clone()), BoardItem::Via(v.clone())])
        .apply(&mut b)
        .unwrap();
    assert_eq!(b.segments[0], s1, "the first track stays first");
    assert_eq!(b.vias[0], v);
    assert_eq!(applied.patch.kind, BoardPatchKind::ItemsReplaced);
    assert_eq!(
        applied.patch.items,
        vec![BoardItemId::Segment(id(SEG_1)), BoardItemId::Via(id(VIA_1))]
    );
    applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before);
}

#[test]
fn a_refused_replace_changes_nothing() {
    let b = with_nets();
    let refusals: Vec<(&str, Vec<BoardItem>, BoardCommandError)> = {
        let mut zero_width = segment(&b, SEG_1);
        zero_width.width = 0.0;
        let mut silk = segment(&b, SEG_1);
        silk.layer = "TopSilkscreen".into();
        let mut stray_net = segment(&b, SEG_1);
        stray_net.net = 99;
        let mut nan = segment(&b, SEG_2);
        nan.end.x = f64::NAN;
        let mut big_hole = via(&b);
        big_hole.drill = 0.6;
        let mut gone = segment(&b, SEG_1);
        gone.uuid = id("00000000-0000-0000-0000-00000000dead");
        let footprint = b.footprints[0].clone();
        vec![
            (
                "zero width",
                vec![BoardItem::Segment(zero_width)],
                BoardCommandError::NotPositive {
                    field: "track width",
                },
            ),
            (
                "silk layer",
                vec![BoardItem::Segment(silk)],
                BoardCommandError::NotCopperLayer("TopSilkscreen".into()),
            ),
            (
                "unknown net",
                vec![BoardItem::Segment(stray_net)],
                BoardCommandError::UnknownNet(99),
            ),
            (
                "not finite, after a good one",
                vec![
                    BoardItem::Segment(segment(&b, SEG_1)),
                    BoardItem::Segment(nan),
                ],
                BoardCommandError::NonFinite { field: "position" },
            ),
            (
                "hole as big as the via",
                vec![BoardItem::Via(big_hole)],
                BoardCommandError::HoleTooLarge {
                    hole: 0.6,
                    diameter: 0.6,
                },
            ),
            (
                "gone",
                vec![BoardItem::Segment(gone.clone())],
                BoardCommandError::ItemNotFound(BoardItemId::Segment(gone.uuid)),
            ),
            (
                "twice",
                vec![
                    BoardItem::Segment(segment(&b, SEG_1)),
                    BoardItem::Segment(segment(&b, SEG_1)),
                ],
                BoardCommandError::DuplicateItem(BoardItemId::Segment(id(SEG_1))),
            ),
            (
                "footprint",
                vec![BoardItem::Footprint(footprint)],
                BoardCommandError::NotReplaceable(BoardItemId::Footprint(id(FP_A))),
            ),
        ]
    };
    for (what, items, expected) in refusals {
        let mut edited = b.clone();
        let error = replace(items).apply(&mut edited).unwrap_err();
        assert_eq!(error, expected, "{what}");
        assert_eq!(snapshot(&edited), snapshot(&b), "{what}");
    }
}

#[test]
fn an_unchanged_odd_value_does_not_block_another_edit() {
    // The test board's via has no size and its tracks no width: moving them
    // must still work.
    let mut b = board();
    let mut s = segment(&b, SEG_1);
    s.end.x = 5.0;
    let mut v = via(&b);
    v.position.y = 2.0;
    replace(vec![BoardItem::Segment(s), BoardItem::Via(v)])
        .apply(&mut b)
        .unwrap();
}

#[test]
fn a_replace_is_one_engine_step() {
    let mut engine = BoardEngine::new(with_nets(), 1);
    let before = snapshot(engine.board());
    let mut s = segment(engine.board(), SEG_1);
    s.width = 0.4;
    engine
        .execute(replace(vec![BoardItem::Segment(s)]))
        .unwrap();
    assert_eq!(engine.undo_len(), 1);
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), before);
}

#[test]
fn replacing_a_pad_undoes_exactly_and_refuses_a_stray_net() {
    let mut b = with_nets();
    let before = snapshot(&b);
    let mut pad = b.footprints[0].pads[1].clone();
    pad.size = Point { x: 1.2, y: 0.1 + 0.9 };
    pad.net = Some(signex_types::pcb::PadNet {
        number: 7,
        name: "GND".into(),
    });
    let applied = BoardCommand::ReplacePads {
        pads: vec![FootprintPad {
            footprint: id(FP_A),
            pad: pad.clone(),
        }],
    }
    .apply(&mut b)
    .unwrap();
    assert_eq!(b.footprints[0].pads[1], pad);
    assert_eq!(applied.patch.kind, BoardPatchKind::PadsReplaced);
    applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before);

    let mut stray = pad;
    stray.net = Some(signex_types::pcb::PadNet {
        number: 7,
        name: "VCC".into(),
    });
    let error = BoardCommand::ReplacePads {
        pads: vec![FootprintPad {
            footprint: id(FP_A),
            pad: stray,
        }],
    }
    .apply(&mut b)
    .unwrap_err();
    assert_eq!(error, BoardCommandError::UnknownNet(7));
    assert_eq!(snapshot(&b), before);
}
