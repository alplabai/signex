//! The move fold weighs only the wires a move changed. These tests hold it to
//! the pass that weighs every wire (`full_scan_moves`): the sheet it leaves
//! and the patch it reports must be equal, exactly, for commits and for drag
//! ghosts, on seeded random sheets full of lined-up wires, buses, child sheets,
//! coloured and foreign-column wires, and on the corpus.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::rubber_band_tests::{Lcg, add_dot, add_label, add_resistor, add_wire};
use crate::test_support::test_sheet;
use crate::{Command, Engine, MovePreview, PreWires};
use signex_types::schematic::{
    Bus, BusEntry, ChildSheet, FillType, NoConnect, Point, SchematicSheet, SelectedItem,
    SelectedKind, SheetPin, Wire,
};
use signex_types::theme::Color;
use uuid::Uuid;

type Moves = Vec<(SelectedItem, f64, f64)>;

const SEEDS: u64 = 800;
const MOVES_PER_SEED: u64 = 4;

/// The whole sheet, field by field: serde writes every saved field, in a
/// fixed key order. A uuid the move minted (a dot, a wire the rubber band
/// adds) is random in each run, so each is written as its number in order of
/// appearance; every uuid of `original` is kept as it is.
pub(crate) fn exact(sheet: &SchematicSheet, original: &SchematicSheet) -> String {
    fn walk(
        value: &mut serde_json::Value,
        known: &HashSet<String>,
        minted: &mut HashMap<String, usize>,
    ) {
        match value {
            serde_json::Value::String(text) => {
                if uuid::Uuid::parse_str(text).is_ok() && !known.contains(text.as_str()) {
                    let next = minted.len();
                    let n = *minted.entry(text.clone()).or_insert(next);
                    *text = format!("minted-{n}");
                }
            }
            serde_json::Value::Array(items) => {
                items.iter_mut().for_each(|v| walk(v, known, minted));
            }
            serde_json::Value::Object(map) => {
                map.values_mut().for_each(|v| walk(v, known, minted));
            }
            _ => {}
        }
    }
    let known: HashSet<String> = {
        let text = serde_json::to_string(original).expect("sheet serialises");
        text.match_indices('"')
            .filter_map(|(at, _)| text.get(at + 1..at + 37))
            .filter(|s| uuid::Uuid::parse_str(s).is_ok())
            .map(str::to_string)
            .collect()
    };
    let mut value = serde_json::to_value(sheet).expect("sheet serialises");
    walk(&mut value, &known, &mut HashMap::new());
    value.to_string()
}

fn lattice(rng: &mut Lcg) -> (f64, f64) {
    (
        10.0 + 5.0 * rng.next(8) as f64,
        10.0 + 5.0 * rng.next(6) as f64,
    )
}

/// A sheet, its net colours set on it (`SchematicSheet::net_colors`).
struct Rig {
    sheet: SchematicSheet,
}

const RED: Color = Color::new(255, 0, 0, 255);
const BLUE: Color = Color::new(0, 0, 255, 255);

fn foreign_column() -> signex_types::format::UnknownColumns {
    serde_json::from_value(serde_json::json!([["x", "1"]])).expect("columns")
}

/// A bus with an entry on it and a wire from the entry's far end.
fn add_bus(sheet: &mut SchematicSheet, rng: &mut Lcg) {
    let (x, y) = lattice(rng);
    sheet.buses.push(Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(x, y),
        end: Point::new(x, y + 10.0),
    });
    sheet.bus_entries.push(BusEntry {
        uuid: Uuid::now_v7(),
        position: Point::new(x, y + 5.0),
        size: (5.0, 0.0),
    });
    add_wire(sheet, (x + 5.0, y + 5.0), (x + 10.0, y + 5.0));
}

/// A child sheet with two pins and a wire from each pin.
fn add_child(sheet: &mut SchematicSheet, rng: &mut Lcg) {
    let (x, y) = lattice(rng);
    let pins: Vec<SheetPin> = (0..2)
        .map(|i| SheetPin {
            uuid: Uuid::now_v7(),
            name: format!("P{i}"),
            direction: "input".to_string(),
            position: Point::new(x, y + 5.0 * i as f64),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        })
        .collect();
    for pin in &pins {
        add_wire(
            sheet,
            (pin.position.x - 5.0, pin.position.y),
            (pin.position.x, pin.position.y),
        );
    }
    sheet.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "Child".to_string(),
        filename: "child.snxsch".to_string(),
        position: Point::new(x, y - 2.0),
        size: (10.0, 10.0),
        stroke_width: 0.12,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins,
        instances: Vec::new(),
    });
}

/// A sheet of short wires on a 5 mm lattice (so moved wires line up with
/// their neighbours and fold), resistors with wires on their pins, labels,
/// dots, no-connects, buses with entries, child sheets with pins; wires with
/// their own colour, a net colour or foreign columns, zero-length wires and
/// exact duplicates.
fn random_rig(rng: &mut Lcg) -> Rig {
    let mut sheet = test_sheet();
    for n in 0..rng.next(4) {
        let at = Point::new(20.0 + 20.0 * n as f64, 20.0 + 10.0 * rng.next(3) as f64);
        add_resistor(&mut sheet, &format!("R{}", n + 1), at);
        if rng.next(2) == 0 {
            add_wire(&mut sheet, (at.x + 5.0, at.y), (at.x + 10.0, at.y));
        }
    }
    for _ in 0..4 + rng.next(14) {
        let (x, y) = lattice(rng);
        let len = 5.0 * (1 + rng.next(2)) as f64;
        let end = if rng.next(2) == 0 {
            (x + len, y)
        } else {
            (x, y + len)
        };
        add_wire(&mut sheet, (x, y), end);
        // A chain: the next wire starts where this one ends.
        if rng.next(3) == 0 {
            let more = (end.0 + (end.0 - x), end.1 + (end.1 - y));
            add_wire(&mut sheet, end, more);
        }
        if rng.next(8) == 0 {
            add_wire(&mut sheet, (x, y), (x, y));
        }
    }
    if rng.next(3) == 0 {
        add_bus(&mut sheet, rng);
    }
    if rng.next(4) == 0 {
        add_child(&mut sheet, rng);
    }
    for i in 0..rng.next(4) {
        add_label(&mut sheet, &format!("N{i}"), lattice(rng));
    }
    for _ in 0..rng.next(4) {
        add_dot(&mut sheet, lattice(rng), rng.next(2) == 0);
    }
    for _ in 0..rng.next(3) {
        let (x, y) = lattice(rng);
        sheet.no_connects.push(NoConnect {
            uuid: Uuid::now_v7(),
            position: Point::new(x, y),
        });
    }
    // An exact duplicate of a wire, overlapping it end to end.
    if rng.next(5) == 0 && !sheet.wires.is_empty() {
        let at = rng.next(sheet.wires.len() as u64) as usize;
        let copy = Wire {
            uuid: Uuid::now_v7(),
            ..sheet.wires[at].clone()
        };
        sheet.wires.push(copy);
    }
    let mut colors = HashMap::new();
    for wire in &mut sheet.wires {
        match rng.next(12) {
            0 => wire.color = Some(RED),
            1 => wire.color = Some(BLUE),
            2 => wire.unknown_columns = foreign_column(),
            3 => {
                colors.insert(wire.uuid, RED);
            }
            4 => {
                colors.insert(wire.uuid, BLUE);
            }
            _ => {}
        }
    }
    if let Some(wire) = sheet.wires.first_mut() {
        wire.locked = rng.next(8) == 0;
    }
    if rng.next(6) == 0
        && let Some(wire) = sheet.wires.last_mut()
    {
        wire.stroke_width = 0.5;
    }
    // Net colours live on the sheet, keyed by wire uuid.
    sheet.net_colors = colors;
    Rig { sheet }
}

/// Every selectable item of the sheet that moves.
fn movable(sheet: &SchematicSheet) -> Vec<SelectedItem> {
    let item = SelectedItem::new;
    let mut items = Vec::new();
    items.extend(sheet.wires.iter().map(|w| item(w.uuid, SelectedKind::Wire)));
    items.extend(
        sheet
            .symbols
            .iter()
            .map(|s| item(s.uuid, SelectedKind::Symbol)),
    );
    items.extend(
        sheet
            .labels
            .iter()
            .map(|l| item(l.uuid, SelectedKind::Label)),
    );
    items.extend(
        sheet
            .junctions
            .iter()
            .map(|j| item(j.uuid, SelectedKind::Junction)),
    );
    items.extend(sheet.buses.iter().map(|b| item(b.uuid, SelectedKind::Bus)));
    items.extend(
        sheet
            .bus_entries
            .iter()
            .map(|b| item(b.uuid, SelectedKind::BusEntry)),
    );
    items.extend(
        sheet
            .no_connects
            .iter()
            .map(|n| item(n.uuid, SelectedKind::NoConnect)),
    );
    items.extend(
        sheet
            .child_sheets
            .iter()
            .map(|c| item(c.uuid, SelectedKind::ChildSheet)),
    );
    items
}

fn random_moves(rng: &mut Lcg, sheet: &SchematicSheet) -> Moves {
    let items = movable(sheet);
    if items.is_empty() {
        return Vec::new();
    }
    let (dx, dy) = match rng.next(4) {
        0 => (5.0, 0.0),
        1 => (-5.0, 0.0),
        2 => (0.0, 5.0 * (1.0 - 2.0 * rng.next(2) as f64)),
        _ => (
            5.0 * (rng.next(5) as f64 - 2.0),
            5.0 * (rng.next(5) as f64 - 2.0),
        ),
    };
    let mut picked: Moves = Vec::new();
    for _ in 0..1 + rng.next(3) {
        let item = items[rng.next(items.len() as u64) as usize];
        if picked.iter().all(|(p, _, _)| p.uuid != item.uuid) {
            picked.push((item, dx, dy));
        }
    }
    picked
}

/// Commit `moves` both ways: equal sheets, equal results, equal warnings.
/// Returns whether wires were folded.
fn commit_both_ways(rig: &Rig, moves: &Moves, keep_wires: bool, what: &str) -> bool {
    let run = |full_scan: bool| {
        let mut engine = Engine::new(rig.sheet.clone()).expect("engine");
        engine.full_scan_moves = full_scan;
        let result = engine.execute(Command::MoveEach {
            moves: moves.clone(),
            keep_wires,
        });
        let result = result.map_err(|e| e.to_string());
        (
            exact(engine.document(), &rig.sheet),
            result,
            engine.warnings.clone(),
            engine.document().wires.len(),
        )
    };
    let (full, near) = (run(true), run(false));
    assert_eq!(near.0, full.0, "commit sheet differs: {what}");
    assert_eq!(near.1, full.1, "commit result differs: {what}");
    assert_eq!(near.2, full.2, "commit warnings differ: {what}");
    near.3 < rig.sheet.wires.len()
}

/// The ghost both ways, from one preview of the sheet. Returns whether wires
/// were folded.
fn ghost_both_ways(rig: &Rig, moves: &Moves, keep_wires: bool, what: &str) -> bool {
    let mut preview = MovePreview::new(&rig.sheet);
    let near = preview.moved_with(moves, keep_wires);
    preview.full_scan_moves = true;
    let full = preview.moved_with(moves, keep_wires);
    assert_eq!(
        exact(&near.0, &rig.sheet),
        exact(&full.0, &rig.sheet),
        "ghost sheet differs: {what}"
    );
    assert_eq!(near.1, full.1, "ghost held differs: {what}");
    near.0.wires.len() < rig.sheet.wires.len()
}

#[test]
fn the_local_fold_equals_the_full_scan_on_random_sheets() {
    let (mut folded, mut ghost_folded, mut moved) = (0, 0, 0);
    for seed in 0..SEEDS {
        let mut rng = Lcg(seed + 1);
        let rig = random_rig(&mut rng);
        for _ in 0..MOVES_PER_SEED {
            let moves = random_moves(&mut rng, &rig.sheet);
            let keep_wires = rng.next(4) != 0;
            let what = format!("seed {seed} keep_wires {keep_wires} moves {moves:?}");
            folded += usize::from(commit_both_ways(&rig, &moves, keep_wires, &what));
            ghost_folded += usize::from(ghost_both_ways(&rig, &moves, keep_wires, &what));
            moved += 1;
        }
    }
    eprintln!("random sheets: {moved} moves, {folded} commits and {ghost_folded} ghosts folded");
    // A test that never folds proves nothing.
    assert!(folded >= 100, "only {folded} of {moved} moves folded wires");
    assert!(
        ghost_folded >= 100,
        "only {ghost_folded} ghosts folded wires"
    );
}

/// A drag: the same preview serves a run of cursor steps, for a wire, a
/// symbol (its pin tips run the rubber-band guard) and a child sheet.
#[test]
fn the_local_fold_equals_the_full_scan_over_a_drag() {
    let (mut steps, mut folded, mut symbol_steps, mut symbol_folded) = (0, 0, 0, 0);
    for seed in 0..200u64 {
        let mut rng = Lcg(seed + 7919);
        let rig = random_rig(&mut rng);
        let sheet = &rig.sheet;
        let wire = (sheet.wires.iter().find(|w| !w.locked))
            .map(|w| SelectedItem::new(w.uuid, SelectedKind::Wire));
        let symbol =
            (sheet.symbols.first()).map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol));
        let child = (sheet.child_sheets.first())
            .map(|c| SelectedItem::new(c.uuid, SelectedKind::ChildSheet));
        let preview = MovePreview::new(sheet);
        let mut full = MovePreview::new(sheet);
        full.full_scan_moves = true;
        for target in [wire, symbol, child].into_iter().flatten() {
            for step in 1..=8 {
                let moves = vec![(target, 5.0 * step as f64, 5.0 * (step % 3) as f64)];
                let a = preview.moved_with(&moves, true);
                let b = full.moved_with(&moves, true);
                assert_eq!(
                    exact(&a.0, sheet),
                    exact(&b.0, sheet),
                    "seed {seed} {target:?} step {step}"
                );
                assert_eq!(a.1, b.1);
                let did_fold = a.0.wires.len() < sheet.wires.len();
                steps += 1;
                folded += usize::from(did_fold);
                if target.kind == SelectedKind::Symbol {
                    symbol_steps += 1;
                    symbol_folded += usize::from(did_fold);
                }
            }
        }
    }
    eprintln!(
        "drags: {steps} steps ({symbol_steps} symbol), {folded} folded ({symbol_folded} symbol)"
    );
    assert!(folded >= 50, "only {folded} of {steps} drag steps folded");
    assert!(symbol_steps >= 400, "only {symbol_steps} symbol drag steps");
    assert!(
        symbol_folded >= 10,
        "only {symbol_folded} symbol drags folded"
    );
}

pub(crate) fn corpus_sheets() -> Vec<std::path::PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut paths = Vec::new();
    for dir in [
        "signex-types/tests/corpus/schematic",
        "signex-app/assets/sample-project",
    ] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        paths.extend(
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "snxsch")),
        );
    }
    paths.sort();
    paths
}

/// Moves that put one wire end on another's along their shared line.
fn lining_up_moves(sheet: &SchematicSheet, limit: usize) -> Vec<Moves> {
    let mut out = Vec::new();
    for a in &sheet.wires {
        for b in sheet.wires.iter().filter(|b| b.uuid != a.uuid) {
            for pa in [a.start, a.end] {
                for pb in [b.start, b.end] {
                    let (dx, dy) = (pa.x - pb.x, pa.y - pb.y);
                    let along_x = dy == 0.0 && a.start.y == a.end.y && b.start.y == b.end.y;
                    let along_y = dx == 0.0 && a.start.x == a.end.x && b.start.x == b.end.x;
                    if (along_x || along_y) && (dx != 0.0 || dy != 0.0) {
                        let item = SelectedItem::new(b.uuid, SelectedKind::Wire);
                        out.push(vec![(item, dx, dy)]);
                    }
                }
            }
        }
        if out.len() >= limit {
            break;
        }
    }
    out.truncate(limit);
    out
}

#[test]
fn the_local_fold_equals_the_full_scan_on_the_corpus() {
    let paths = corpus_sheets();
    assert!(
        paths.len() >= 30,
        "corpus not found: {} sheets",
        paths.len()
    );
    let (mut with_wires, mut tried, mut folded) = (0, 0, 0);
    for path in &paths {
        let Ok(engine) = Engine::open(path) else {
            continue;
        };
        let rig = Rig {
            sheet: engine.document().clone(),
        };
        let sheet = &rig.sheet;
        with_wires += usize::from(!sheet.wires.is_empty());
        let mut rng = Lcg(sheet.wires.len() as u64 + 31);
        for trial in 0..24 {
            let mut moves = random_moves(&mut rng, sheet);
            // Corpus sheets are off the lattice: also try each wire alone.
            if let Some(wire) = sheet.wires.get(trial) {
                let dx = 2.54 * (rng.next(5) as f64 - 2.0);
                let dy = 2.54 * (rng.next(5) as f64 - 2.0);
                moves = vec![(SelectedItem::new(wire.uuid, SelectedKind::Wire), dx, dy)];
            }
            let what = format!("{} trial {trial}", path.display());
            commit_both_ways(&rig, &moves, true, &what);
            ghost_both_ways(&rig, &moves, true, &what);
        }
        // Moves that line a wire up with a neighbour, so they fold.
        for (n, moves) in lining_up_moves(sheet, 150).into_iter().enumerate() {
            let what = format!("{} lining up {n}", path.display());
            tried += 1;
            folded += usize::from(commit_both_ways(&rig, &moves, true, &what));
            ghost_both_ways(&rig, &moves, true, &what);
        }
    }
    eprintln!("corpus: {with_wires} sheets with wires, {tried} lining-up moves, {folded} folded");
    assert!(
        with_wires >= 3,
        "only {with_wires} corpus sheets have wires"
    );
    assert!(
        folded >= 2,
        "only {folded} of {tried} lining-up moves folded"
    );
}

/// Two wires of the pre-move sheet share a uuid: the pairs cannot be
/// answered by uuid, so the local fold hands over to the full scan.
#[test]
fn pre_move_wires_sharing_a_uuid_fall_back_to_the_full_scan() {
    // The guard checks its incremental nets against the full derive, which a
    // repeated id makes the base give up on; that is not under test here.
    let _check_off = crate::transform::rubber_band::self_check::off();
    let (mut folded, mut moved) = (0, 0);
    for seed in 0..300u64 {
        let mut rng = Lcg(seed + 104_729);
        let mut rig = random_rig(&mut rng);
        if rig.sheet.wires.len() < 2 {
            continue;
        }
        let other = rig.sheet.wires[0].uuid;
        rig.sheet.wires[1].uuid = other;
        assert!(
            PreWires::new(&rig.sheet.wires)
                .changed_keys(&rig.sheet.wires)
                .is_none()
        );
        for _ in 0..3 {
            let moves = random_moves(&mut rng, &rig.sheet);
            let what = format!("shared uuid, seed {seed} moves {moves:?}");
            folded += usize::from(commit_both_ways(&rig, &moves, true, &what));
            ghost_both_ways(&rig, &moves, true, &what);
            moved += 1;
        }
    }
    eprintln!("shared uuid: {moved} moves, {folded} folded");
    assert!(moved >= 300 && folded >= 10, "{folded} of {moved} folded");
}

/// A move leaves two wires of one uuid among wires that had unique ones, or
/// a uuid that was already among the pre-move wires: the local fold hands
/// over to the full scan and folds what it folds.
#[test]
fn a_uuid_repeated_after_the_move_falls_back_to_the_full_scan() {
    let (mut fell_back, mut folded) = (0, 0);
    for seed in 0..300u64 {
        let mut rng = Lcg(seed + 15_485_863);
        let rig = random_rig(&mut rng);
        let wires = &rig.sheet.wires;
        let Some(unlocked) = wires.iter().find(|w| !w.locked) else {
            continue;
        };
        let (moved_uuid, _) = (unlocked.uuid, ());
        let pre = PreWires::new(wires);
        let run = |full_scan: bool| {
            let mut engine = Engine::new(rig.sheet.clone()).expect("engine");
            let before = engine.touching_wire_pairs();
            let item = SelectedItem::new(moved_uuid, SelectedKind::Wire);
            engine.move_selected_item(&item, 5.0, 0.0);
            // What a rubber band that reused a uuid would leave: a wire with a
            // uuid of the pre-move sheet, lined up with another.
            let echo = Wire {
                uuid: wires[wires.len() - 1].uuid,
                start: Point::new(wires[0].end.x, wires[0].end.y),
                end: Point::new(wires[0].end.x + 5.0, wires[0].end.y),
                ..Default::default()
            };
            engine.document.wires.push(echo);
            let repeated = pre.changed_keys(&engine.document.wires).is_none();
            let merged = if full_scan {
                engine.merge_collinear_wires(&before)
            } else {
                engine.merge_collinear_wires_near(&pre)
            };
            (exact(engine.document(), &rig.sheet), repeated, merged)
        };
        let (near, full) = (run(false), run(true));
        assert_eq!(near.0, full.0, "seed {seed}");
        assert_eq!(near.2, full.2, "seed {seed}");
        fell_back += usize::from(near.1);
        folded += usize::from(near.2);
    }
    eprintln!("repeated uuid: {fell_back} fell back, {folded} folded");
    assert!(
        fell_back >= 100 && folded >= 5,
        "{fell_back} fell back, {folded} folded"
    );
}
