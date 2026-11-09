//! The move fold weighs only the wires a move changed. These tests hold it to
//! the pass that weighs every wire (`full_scan_moves`): the sheet it leaves
//! and the patch it reports must be equal, exactly, for commits and for drag
//! ghosts, on seeded random sheets full of lined-up wires and on the corpus.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::rubber_band_tests::{Lcg, add_dot, add_label, add_resistor, add_wire};
use crate::test_support::test_sheet;
use crate::{Command, Engine, MovePreview};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};

type Moves = Vec<(SelectedItem, f64, f64)>;

const SEEDS: u64 = 800;
const MOVES_PER_SEED: u64 = 4;

/// The whole sheet, field by field: serde writes every saved field, in a
/// fixed key order. A uuid the move minted (a dot, a wire the rubber band
/// adds) is random in each run, so each is written as its number in order of
/// appearance; every uuid of `original` is kept as it is.
fn exact(sheet: &SchematicSheet, original: &SchematicSheet) -> String {
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

/// A sheet of short wires on a 5 mm lattice (so moved wires line up with
/// their neighbours and fold), resistors with wires on their pins, labels
/// and dots.
fn random_sheet(rng: &mut Lcg) -> SchematicSheet {
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
    }
    for i in 0..rng.next(4) {
        add_label(&mut sheet, &format!("N{i}"), lattice(rng));
    }
    for _ in 0..rng.next(4) {
        add_dot(&mut sheet, lattice(rng), rng.next(2) == 0);
    }
    if let Some(wire) = sheet.wires.first_mut() {
        wire.locked = rng.next(8) == 0;
    }
    if rng.next(6) == 0
        && let Some(wire) = sheet.wires.last_mut()
    {
        wire.stroke_width = 0.5;
    }
    sheet
}

/// Every selectable item of the sheet that moves.
fn movable(sheet: &SchematicSheet) -> Vec<SelectedItem> {
    let mut items = Vec::new();
    items.extend((sheet.wires.iter()).map(|w| SelectedItem::new(w.uuid, SelectedKind::Wire)));
    items.extend((sheet.symbols.iter()).map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol)));
    items.extend((sheet.labels.iter()).map(|l| SelectedItem::new(l.uuid, SelectedKind::Label)));
    items.extend(
        (sheet.junctions.iter()).map(|j| SelectedItem::new(j.uuid, SelectedKind::Junction)),
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
fn commit_both_ways(sheet: &SchematicSheet, moves: &Moves, keep_wires: bool, what: &str) -> bool {
    let run = |full_scan: bool| {
        let mut engine = Engine::new(sheet.clone()).expect("engine");
        engine.full_scan_moves = full_scan;
        let result = engine.execute(Command::MoveEach {
            moves: moves.clone(),
            keep_wires,
        });
        let result = result.map_err(|e| e.to_string());
        (
            exact(engine.document(), sheet),
            result,
            engine.warnings.clone(),
            engine.document().wires.len(),
        )
    };
    let (full, near) = (run(true), run(false));
    assert_eq!(near.0, full.0, "commit sheet differs: {what}");
    assert_eq!(near.1, full.1, "commit result differs: {what}");
    assert_eq!(near.2, full.2, "commit warnings differ: {what}");
    near.3 < sheet.wires.len()
}

/// The ghost both ways, from one preview of the sheet.
fn ghost_both_ways(sheet: &SchematicSheet, moves: &Moves, keep_wires: bool, what: &str) {
    let mut preview = MovePreview::new(sheet);
    let near = preview.moved_with(moves, keep_wires, &Default::default());
    preview.full_scan_moves = true;
    let full = preview.moved_with(moves, keep_wires, &Default::default());
    assert_eq!(
        exact(&near.0, sheet),
        exact(&full.0, sheet),
        "ghost sheet differs: {what}"
    );
    assert_eq!(near.1, full.1, "ghost held differs: {what}");
}

#[test]
fn the_local_fold_equals_the_full_scan_on_random_sheets() {
    let (mut folded, mut moved) = (0, 0);
    for seed in 0..SEEDS {
        let mut rng = Lcg(seed + 1);
        let sheet = random_sheet(&mut rng);
        for _ in 0..MOVES_PER_SEED {
            let moves = random_moves(&mut rng, &sheet);
            let keep_wires = rng.next(4) != 0;
            let what = format!("seed {seed} keep_wires {keep_wires} moves {moves:?}");
            folded += usize::from(commit_both_ways(&sheet, &moves, keep_wires, &what));
            ghost_both_ways(&sheet, &moves, keep_wires, &what);
            moved += 1;
        }
    }
    eprintln!("local fold == full scan on {moved} moves, {folded} of them folded wires");
    // A test that never folds proves nothing.
    assert!(folded >= 100, "only {folded} of {moved} moves folded wires");
}

/// A drag: the same preview serves a run of cursor steps.
#[test]
fn the_local_fold_equals_the_full_scan_over_a_drag() {
    for seed in 0..200u64 {
        let mut rng = Lcg(seed + 7919);
        let sheet = random_sheet(&mut rng);
        let item = movable(&sheet);
        let Some(&target) = item.first() else {
            continue;
        };
        let near = MovePreview::new(&sheet);
        let mut full = MovePreview::new(&sheet);
        full.full_scan_moves = true;
        for step in 1..=8 {
            let moves = vec![(target, 5.0 * step as f64, 5.0 * (step % 3) as f64)];
            let (a, b) = (
                near.moved_with(&moves, true, &Default::default()),
                full.moved_with(&moves, true, &Default::default()),
            );
            assert_eq!(
                exact(&a.0, &sheet),
                exact(&b.0, &sheet),
                "seed {seed} step {step}"
            );
            assert_eq!(a.1, b.1);
        }
    }
}

fn corpus_sheets() -> Vec<std::path::PathBuf> {
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

#[test]
fn the_local_fold_equals_the_full_scan_on_the_corpus() {
    let paths = corpus_sheets();
    assert!(
        paths.len() >= 30,
        "corpus not found: {} sheets",
        paths.len()
    );
    let mut with_wires = 0;
    for path in &paths {
        let Ok(engine) = Engine::open(path) else {
            continue;
        };
        let sheet = engine.document().clone();
        with_wires += usize::from(!sheet.wires.is_empty());
        let mut rng = Lcg(sheet.wires.len() as u64 + 31);
        for trial in 0..24 {
            let mut moves = random_moves(&mut rng, &sheet);
            // Corpus sheets are off the lattice: also try each wire alone.
            if let Some(wire) = sheet.wires.get(trial) {
                let dx = 2.54 * (rng.next(5) as f64 - 2.0);
                let dy = 2.54 * (rng.next(5) as f64 - 2.0);
                moves = vec![(SelectedItem::new(wire.uuid, SelectedKind::Wire), dx, dy)];
            }
            let what = format!("{} trial {trial}", path.display());
            commit_both_ways(&sheet, &moves, true, &what);
            ghost_both_ways(&sheet, &moves, true, &what);
        }
    }
    assert!(
        with_wires >= 3,
        "only {with_wires} corpus sheets have wires"
    );
}
