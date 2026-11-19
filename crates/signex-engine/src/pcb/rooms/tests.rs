//! #2457: room commands and Create Rooms from Sheets.

use serde_json::{Value, json};
use signex_types::pcb::{PcbBoard, Point, Room, RoomLayer, RoomOutline, RoomSource};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::*;
use crate::pcb::{BoardEngine, BoardPatchKind};

const SHEET_A: u128 = 0xA;
const SHEET_B: u128 = 0xB;

fn sheet(chain: &[u128]) -> InstancePath {
    InstancePath::from_chain(chain.iter().map(|&n| Uuid::from_u128(n)).collect())
}

/// A part with no pads or courtyard: its body is the point at its origin.
fn part(n: u128, x: f64, y: f64, on: Option<&[u128]>) -> Value {
    let link = on.map(|chain| {
        json!({
            "sheet": chain.iter().map(|&s| Uuid::from_u128(s)).collect::<Vec<_>>(),
            "units": [Uuid::from_u128(1000 + n)],
        })
    });
    json!({
        "uuid": Uuid::from_u128(n),
        "reference": format!("R{n}"),
        "position": {"x": x, "y": y},
        "layer": "TopCopper",
        "schematic_link": link,
    })
}

/// A 60 by 40 mm board: R1 and R2 on sheet A, R3 on sheet B, R4 on no sheet.
fn board() -> PcbBoard {
    serde_json::from_value(json!({
        "uuid": Uuid::nil(),
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 60.0, "y": 0.0},
            {"x": 60.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ],
        "footprints": [
            part(1, 5.1, 5.2, Some(&[SHEET_A])),
            part(2, 9.3, 7.7, Some(&[SHEET_A, 0x77])),
            part(3, 40.0, 20.0, Some(&[SHEET_B])),
            part(4, 30.0, 30.0, None),
        ],
    }))
    .unwrap()
}

fn room(n: u128, outline: RoomOutline, on: u128) -> Room {
    let mut room = Room::drawn(Uuid::from_u128(n), format!("room{n}"), "#A058C8", outline);
    room.source = Some(RoomSource {
        sheet: sheet(&[on]),
        channel: None,
    });
    room
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> RoomOutline {
    RoomOutline::Rect {
        x_mm: x,
        y_mm: y,
        w_mm: w,
        h_mm: h,
    }
}

fn add(engine: &mut BoardEngine, room: Room) {
    engine
        .execute(BoardCommand::AddRoom {
            room: Box::new(room),
        })
        .expect("adds");
}

fn position(board: &PcbBoard, n: u128) -> Point {
    let id = Uuid::from_u128(n);
    board
        .footprints
        .iter()
        .find(|fp| fp.uuid == id)
        .unwrap()
        .position
}

#[test]
fn a_room_holds_the_parts_of_its_sheet_and_the_sheets_below_it() {
    let b = board();
    let a = room(100, rect(0.0, 0.0, 20.0, 20.0), SHEET_A);
    let held: Vec<Uuid> = a.members(&b.footprints).map(|fp| fp.uuid).collect();
    assert_eq!(held, vec![Uuid::from_u128(1), Uuid::from_u128(2)]);
}

#[test]
fn adding_and_removing_a_room_are_undone_exactly() {
    let mut engine = BoardEngine::new(board(), 1);
    let r = room(100, rect(1.0, 2.0, 3.0, 4.0), SHEET_A);
    let patch = engine
        .execute(BoardCommand::AddRoom {
            room: Box::new(r.clone()),
        })
        .expect("adds");
    assert_eq!(patch.kind, BoardPatchKind::RoomsChanged);
    engine
        .execute(BoardCommand::RemoveRoom { room: r.uuid })
        .expect("removes");
    assert!(engine.board().rooms.is_empty());
    engine.undo().expect("undo");
    assert_eq!(engine.board().rooms, vec![r]);
    engine.undo().expect("undo");
    assert!(engine.board().rooms.is_empty());
}

#[test]
fn a_room_that_breaks_a_rule_is_refused() {
    let mut engine = BoardEngine::new(board(), 1);
    let mut flat = room(100, rect(0.0, 0.0, 0.0, 4.0), SHEET_A);
    assert_eq!(
        engine.execute(BoardCommand::AddRoom {
            room: Box::new(flat.clone())
        }),
        Err(BoardCommandError::RoomInvalid { field: "outline" })
    );
    flat.outline = rect(0.0, 0.0, 1.0, 1.0);
    flat.colour = "purple".into();
    assert_eq!(
        engine.execute(BoardCommand::AddRoom {
            room: Box::new(flat)
        }),
        Err(BoardCommandError::RoomInvalid { field: "colour" })
    );
    assert_eq!(engine.undo_len(), 0);
}

#[test]
fn moving_a_room_moves_its_members_in_one_undo_step() {
    let mut engine = BoardEngine::new(board(), 1);
    add(&mut engine, room(100, rect(1.0, 1.0, 20.0, 20.0), SHEET_A));
    let before = engine.board().clone();
    engine
        .execute(BoardCommand::MoveRoom {
            room: Uuid::from_u128(100),
            to: Point::new(31.0, 11.0),
        })
        .expect("moves");
    let after = engine.board();
    assert_eq!(after.rooms[0].outline, rect(31.0, 11.0, 20.0, 20.0));
    assert_eq!(position(after, 1), Point::new(5.1 + 30.0, 5.2 + 10.0));
    assert_eq!(position(after, 2), Point::new(9.3 + 30.0, 7.7 + 10.0));
    assert_eq!(position(after, 3), position(&before, 3), "not a member");
    assert_eq!(position(after, 4), position(&before, 4), "on no sheet");
    assert_eq!(engine.undo_len(), 2, "the add and the move");
    engine.undo().expect("undo");
    let undone = engine.board();
    assert_eq!(undone.rooms, before.rooms);
    for n in 1..=4 {
        let (a, b) = (position(undone, n), position(&before, n));
        assert_eq!(
            (a.x.to_bits(), a.y.to_bits()),
            (b.x.to_bits(), b.y.to_bits())
        );
    }
}

#[test]
fn a_locked_room_refuses_a_move_and_a_fit() {
    let mut engine = BoardEngine::new(board(), 1);
    let mut r = room(100, rect(1.0, 1.0, 20.0, 20.0), SHEET_A);
    r.locked = true;
    add(&mut engine, r);
    let id = Uuid::from_u128(100);
    assert_eq!(
        engine.execute(BoardCommand::MoveRoom {
            room: id,
            to: Point::new(30.0, 10.0)
        }),
        Err(BoardCommandError::RoomLocked(id))
    );
    assert_eq!(
        engine.execute(BoardCommand::FitRoomToParts { room: id }),
        Err(BoardCommandError::RoomLocked(id))
    );
    assert_eq!(position(engine.board(), 1), Point::new(5.1, 5.2));
    assert_eq!(engine.undo_len(), 1, "only the add");
}

#[test]
fn a_fit_wraps_the_members_plus_the_margin() {
    let mut engine = BoardEngine::new(board(), 1);
    let mut r = room(100, rect(30.0, 30.0, 5.0, 5.0), SHEET_A);
    r.margin_mm = 1.0;
    add(&mut engine, r);
    engine
        .execute(BoardCommand::FitRoomToParts {
            room: Uuid::from_u128(100),
        })
        .expect("fits");
    let room = &engine.board().rooms[0];
    let b = room_box(room).unwrap();
    assert!(b.holds(&part_box(&engine.board().footprints[0]).inflate(1.0)));
    assert!(b.holds(&part_box(&engine.board().footprints[1]).inflate(1.0)));
    assert!(members_outside(room, engine.board()).is_empty());
}

#[test]
fn pulling_packs_the_members_outside_into_the_room() {
    let mut engine = BoardEngine::new(board(), 1);
    add(&mut engine, room(100, rect(40.0, 2.0, 15.0, 10.0), SHEET_A));
    let id = Uuid::from_u128(100);
    assert_eq!(
        members_outside(&engine.board().rooms[0], engine.board()).len(),
        2
    );
    engine
        .execute(BoardCommand::PullPartsIntoRoom { room: id })
        .expect("pulls");
    let board = engine.board();
    assert!(members_outside(&board.rooms[0], board).is_empty());
    assert_eq!(position(board, 3), Point::new(40.0, 20.0), "not a member");
    engine.undo().expect("undo");
    assert_eq!(position(engine.board(), 1), Point::new(5.1, 5.2));
}

fn sheets() -> Vec<SheetMembers> {
    vec![
        SheetMembers {
            name: "dcdc".into(),
            file: "dcdc.snxsch".into(),
            placements: vec![sheet(&[SHEET_A])],
        },
        SheetMembers {
            name: "amp".into(),
            file: "amp.snxsch".into(),
            placements: vec![sheet(&[SHEET_B])],
        },
    ]
}

fn counter() -> impl FnMut() -> Uuid {
    let mut n = 500_u128;
    move || {
        n += 1;
        Uuid::from_u128(n)
    }
}

#[test]
fn a_room_around_parts_wraps_the_members_on_the_board() {
    let options = RoomOptions {
        move_parts: false,
        ..RoomOptions::default()
    };
    let plan = plan_rooms_from_sheets(&board(), &sheets(), &options, &mut counter());
    assert_eq!(plan.rooms.len(), 2);
    let dcdc = &plan.rooms[0];
    assert_eq!(dcdc.room.name, "dcdc");
    assert_eq!(dcdc.room.colour, "#2CA8B8", "the first palette colour");
    assert_eq!(plan.rooms[1].room.colour, "#C8A02C");
    assert_eq!(dcdc.members.len(), 2);
    assert!(dcdc.room.keep_parts_inside);
    assert_eq!(dcdc.room.layer, RoomLayer::Top);
    let b = room_box(&dcdc.room).unwrap();
    assert!(b.holds(&BoxMm {
        min: Point::new(5.1 - 0.5, 5.2 - 0.5),
        max: Point::new(9.3 + 0.5, 7.7 + 0.5),
    }));
    assert!(plan.moves.is_empty());
}

#[test]
fn an_even_grid_splits_the_board_one_cell_per_room() {
    let options = RoomOptions {
        shape: PlanShape::EvenGrid,
        move_parts: false,
        ..RoomOptions::default()
    };
    let plan = plan_rooms_from_sheets(&board(), &sheets(), &options, &mut counter());
    let region = BoxMm {
        min: Point::ZERO,
        max: Point::new(60.0, 40.0),
    };
    let boxes: Vec<BoxMm> = plan
        .rooms
        .iter()
        .map(|p| room_box(&p.room).unwrap())
        .collect();
    assert!(boxes.iter().all(|b| region.holds(b)));
    assert!(!boxes[0].overlaps(&boxes[1]));
}

#[test]
fn a_sheet_placed_twice_gets_one_room_per_channel() {
    let sheets = vec![SheetMembers {
        name: "amp".into(),
        file: "amp.snxsch".into(),
        placements: vec![sheet(&[SHEET_A]), sheet(&[SHEET_B])],
    }];
    let plan = plan_rooms_from_sheets(&board(), &sheets, &RoomOptions::default(), &mut counter());
    let names: Vec<&str> = plan.rooms.iter().map(|p| p.room.name.as_str()).collect();
    assert_eq!(names, ["amp_CH1", "amp_CH2"]);
    let channels: Vec<Option<u32>> = plan
        .rooms
        .iter()
        .map(|p| p.room.source.as_ref().unwrap().channel)
        .collect();
    assert_eq!(channels, [Some(1), Some(2)]);
    assert_eq!(plan.channel_rooms(), 2);
    assert_eq!(plan.rooms[0].room.colour, plan.rooms[1].room.colour);
}

#[test]
fn a_room_of_the_same_name_is_kept_unless_replacing() {
    let mut b = board();
    let mut old = room(100, rect(1.0, 1.0, 2.0, 2.0), SHEET_A);
    old.name = "dcdc".into();
    old.colour = "#409C58".into();
    b.rooms.push(old.clone());
    let kept = plan_rooms_from_sheets(&b, &sheets(), &RoomOptions::default(), &mut counter());
    assert_eq!(kept.kept, vec!["dcdc".to_string()]);
    assert_eq!(kept.rooms.len(), 1);
    let options = RoomOptions {
        replace_same_name: true,
        ..RoomOptions::default()
    };
    let replaced = plan_rooms_from_sheets(&b, &sheets(), &options, &mut counter());
    assert_eq!(replaced.rooms[0].replaces, Some(old.uuid));
    assert_eq!(replaced.rooms[0].room.colour, "#409C58", "keeps its colour");
    assert!(
        replaced
            .commands()
            .contains(&BoardCommand::RemoveRoom { room: old.uuid })
    );
}

#[test]
fn parts_below_the_board_are_packed_into_their_room_in_one_step() {
    let mut b = board();
    // Update PCB stages new parts below the board.
    b.footprints
        .push(serde_json::from_value(part(5, 3.0, 55.0, Some(&[SHEET_B]))).unwrap());
    let plan = plan_rooms_from_sheets(&b, &sheets(), &RoomOptions::default(), &mut counter());
    let amp = &plan.rooms[1];
    let to = plan
        .moves
        .iter()
        .find(|(id, _)| *id == Uuid::from_u128(5))
        .map(|(_, to)| *to)
        .expect("R5 moves");
    let inner = room_box(&amp.room).unwrap().inflate(-amp.room.margin_mm);
    assert!(inner.holds(&BoxMm { min: to, max: to }));
    let mut engine = BoardEngine::new(b.clone(), 1);
    engine
        .execute_batch(plan.commands())
        .expect("creates the rooms");
    assert_eq!(engine.undo_len(), 1, "one undo step");
    assert_eq!(engine.board().rooms.len(), 2);
    assert_eq!(position(engine.board(), 5), to);
    engine.undo().expect("undo");
    assert!(engine.board().rooms.is_empty());
    assert_eq!(position(engine.board(), 5), Point::new(3.0, 55.0));
}
