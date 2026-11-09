//! The commit path (`Engine::execute`) asks only what is near: the rubber-band
//! pass reads a spatial grid and the junction reconcile indexes only the wires
//! near the touched ones (`transform/region_wires.rs`). These tests hold every
//! command that reconciles (the drop of a move, delete, rotate, mirror, place
//! wire, break wire) to the same command with the local paths off
//! (`full_scan_follow` and `full_scan_guard`): the sheet, the result and its
//! patch, the warnings and the unkept pins must be equal, exactly.
//!
//! The cases are seeded random sheets of parts and of wires, sheets whose wire
//! ends and dots sit a fraction of the junction tolerance either side of a
//! meeting (where the reconcile margin is tested), and every corpus sheet. A
//! floor on each count keeps a test that stops exercising the local paths
//! from passing.

use std::collections::BTreeMap;

use crate::move_fold_local_tests::{corpus_sheets, exact, movable, random_rig};
use crate::rubber_band_tests::{Lcg, add_dot, add_wire, random_case};
use crate::test_support::test_sheet;
use crate::transform::rubber_band::{self_check, tally};
use crate::{Command, Engine, MirrorAxis, SelectionTransform};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Wire};
use uuid::Uuid;

/// Everything a command leaves that a user or a caller can see.
#[derive(Debug, PartialEq)]
struct Outcome {
    sheet: String,
    /// The net colours, a new wire's keyed by its place in the wire list.
    colors: Vec<(String, signex_types::theme::Color)>,
    result: Result<crate::CommandResult, String>,
    warnings: Vec<String>,
    unkept_pins: Vec<String>,
}

/// Per command name: cases, cases whose dots changed, cases whose reconcile
/// indexed fewer wires than the sheet holds.
#[derive(Debug, Default)]
struct Floors {
    by_command: BTreeMap<&'static str, (usize, usize, usize)>,
}

impl Floors {
    fn of(&self, name: &str) -> (usize, usize, usize) {
        self.by_command.get(name).copied().unwrap_or_default()
    }

    fn total(&self) -> (usize, usize, usize) {
        (self.by_command.values()).fold((0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2))
    }
}

/// The command's outcome, what it tallied, and whether it changed the dots.
fn run(
    sheet: &SchematicSheet,
    command: &Command,
    full_scan: bool,
) -> (Outcome, tally::Tally, bool) {
    let mut engine = Engine::new(sheet.clone()).expect("engine");
    engine.full_scan_follow = full_scan;
    engine.full_scan_guard = full_scan;
    let _ = tally::take();
    let result = engine.execute(command.clone());
    let counts = tally::take();
    // A net colour is keyed by wire id, and `exact` renumbers only the ids it
    // finds as values: key a new wire's colour by its place in the list.
    let mut after = engine.document().clone();
    let mut colors: Vec<(String, signex_types::theme::Color)> = (after.net_colors.drain())
        .map(|(id, color)| {
            let known =
                sheet.wires.iter().any(|w| w.uuid == id) || sheet.net_colors.contains_key(&id);
            let key = match after.wires.iter().position(|w| w.uuid == id) {
                Some(at) if !known => format!("new wire {at}"),
                _ => id.to_string(),
            };
            (key, color)
        })
        .collect();
    colors.sort_by(|a, b| a.0.cmp(&b.0));
    let outcome = Outcome {
        sheet: exact(&after, sheet),
        colors,
        result: result.map_err(|e| e.to_string()),
        warnings: engine.warnings.clone(),
        unkept_pins: engine.unkept_pins.clone(),
    };
    let dots_changed = engine.document().junctions != sheet.junctions;
    (outcome, counts, dots_changed)
}

/// Execute `command` on `sheet` both ways and hold the two to each other.
fn both_ways(sheet: &SchematicSheet, name: &'static str, command: Command, floors: &mut Floors) {
    // The differential alone must tell the two apart: no inline cross-check.
    let _off = self_check::off();
    let (local, counts, dots_changed) = run(sheet, &command, false);
    let (full, full_counts, _) = run(sheet, &command, true);
    assert_eq!(local, full, "{name}: {command:?}");
    assert_eq!(full_counts.reconciles_narrowed, 0, "{name}");
    let entry = floors.by_command.entry(name).or_default();
    entry.0 += 1;
    entry.1 += usize::from(dots_changed);
    entry.2 += usize::from(counts.reconciles_narrowed > 0);
}

/// A few items of the sheet, any kind that moves.
fn pick(rng: &mut Lcg, sheet: &SchematicSheet, most: u64) -> Vec<SelectedItem> {
    let items = movable(sheet);
    if items.is_empty() {
        return Vec::new();
    }
    let mut picked: Vec<SelectedItem> = Vec::new();
    for _ in 0..1 + rng.next(most) {
        let item = items[rng.next(items.len() as u64) as usize];
        if !picked.contains(&item) {
            picked.push(item);
        }
    }
    picked
}

/// One wire of the sheet, or none.
fn some_wire<'a>(rng: &mut Lcg, sheet: &'a SchematicSheet) -> Option<&'a Wire> {
    (!sheet.wires.is_empty()).then(|| &sheet.wires[rng.next(sheet.wires.len() as u64) as usize])
}

/// A step of `unit`, now and then nudged by part of the junction tolerance.
fn step(rng: &mut Lcg, unit: f64) -> f64 {
    let base = unit * (rng.next(5) as f64 - 2.0);
    base + nudge(rng)
}

/// Zero, or a distance on either side of one to five junction tolerances.
fn nudge(rng: &mut Lcg) -> f64 {
    let tol = crate::JUNCTION_TOLERANCE_MM;
    let parts = [
        0.0, 0.0, 0.0, 0.4, -0.4, 0.99, -0.99, 1.01, -1.01, 1.5, 2.0, -2.0, 2.99, 3.01, -3.5, 5.0,
    ];
    parts[rng.next(parts.len() as u64) as usize] * tol
}

/// Every command that reconciles, once each, on `sheet`.
fn every_command(rng: &mut Lcg, sheet: &SchematicSheet, unit: f64, floors: &mut Floors) {
    let items = pick(rng, sheet, 3);
    if !items.is_empty() {
        let (dx, dy) = (step(rng, unit), step(rng, unit));
        let moves = items.iter().map(|item| (*item, dx, dy)).collect();
        let keep_wires = rng.next(4) != 0;
        both_ways(
            sheet,
            "drop",
            Command::MoveEach { moves, keep_wires },
            floors,
        );
        let command = Command::MoveSelection {
            items: items.clone(),
            dx,
            dy,
            keep_wires,
        };
        both_ways(sheet, "move", command, floors);
        let command = Command::RotateSelection {
            items: items.clone(),
            angle_degrees: 90.0 * (1 + rng.next(3)) as f64,
            keep_wires: rng.next(3) != 0,
        };
        both_ways(sheet, "rotate", command, floors);
        let axis = [MirrorAxis::Horizontal, MirrorAxis::Vertical][rng.next(2) as usize];
        let command = Command::MirrorSelection {
            items: items.clone(),
            axis,
            keep_wires: rng.next(3) != 0,
        };
        both_ways(sheet, "mirror", command, floors);
        // Rotate and mirror above turn parts and labels in place; this turns
        // wire geometry too, about a point of the sheet.
        let centre = match some_wire(rng, sheet) {
            Some(wire) => wire.start,
            None => Point::new(30.0, 30.0),
        };
        let transform = [
            SelectionTransform::RotateClockwise,
            SelectionTransform::RotateCounterclockwise,
            SelectionTransform::Mirror(MirrorAxis::Horizontal),
            SelectionTransform::Mirror(MirrorAxis::Vertical),
        ][rng.next(4) as usize];
        let command = Command::TransformSelection {
            items: items.clone(),
            centre,
            transform,
            keep_wires: rng.next(3) != 0,
        };
        both_ways(sheet, "transform", command, floors);
    }
    // Delete wires most of the time: that is what strands dots.
    let mut doomed = pick(rng, sheet, 2);
    if let Some(wire) = some_wire(rng, sheet) {
        doomed.push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
    }
    if !doomed.is_empty() {
        both_ways(
            sheet,
            "delete",
            Command::DeleteSelection { items: doomed },
            floors,
        );
    }
    // A wire from an existing wire's end or interior, out by a step.
    let start = match some_wire(rng, sheet) {
        Some(wire) => {
            let t = [0.0, 0.5, 1.0, 0.25][rng.next(4) as usize];
            Point::new(
                wire.start.x + t * (wire.end.x - wire.start.x) + nudge(rng),
                wire.start.y + t * (wire.end.y - wire.start.y) + nudge(rng),
            )
        }
        None => Point::new(20.0, 20.0),
    };
    let end = match rng.next(3) {
        0 => Point::new(start.x + unit * (1 + rng.next(3)) as f64, start.y),
        1 => Point::new(start.x, start.y - unit * (1 + rng.next(3)) as f64),
        _ => Point::new(start.x + step(rng, unit), start.y + step(rng, unit)),
    };
    let wire = Wire {
        uuid: Uuid::now_v7(),
        start,
        end,
        ..Default::default()
    };
    both_ways(
        sheet,
        "place wire",
        Command::PlaceWireSegment { wire },
        floors,
    );
    if let Some(wire) = some_wire(rng, sheet) {
        let t = [0.5, 0.25, 0.75][rng.next(3) as usize];
        let at = Point::new(
            wire.start.x + t * (wire.end.x - wire.start.x),
            wire.start.y + t * (wire.end.y - wire.start.y),
        );
        let command = Command::BreakWire {
            wire: wire.uuid,
            at,
        };
        both_ways(sheet, "break wire", command, floors);
    }
}

/// Wires on a 5 mm lattice whose ends are often a fraction of the junction
/// tolerance off it, stubs ending on (or just off) other wires' interiors,
/// stars, and minted and user dots on and just off those meetings.
fn near_tolerance_sheet(rng: &mut Lcg) -> SchematicSheet {
    let mut sheet = test_sheet();
    let lattice = |rng: &mut Lcg| {
        (
            10.0 + 5.0 * rng.next(10) as f64,
            10.0 + 5.0 * rng.next(8) as f64,
        )
    };
    for _ in 0..6 + rng.next(10) {
        let (x, y) = lattice(rng);
        let len = 5.0 * (1 + rng.next(4)) as f64;
        let (a, b) = if rng.next(2) == 0 {
            ((x, y), (x + len, y + nudge(rng)))
        } else {
            ((x + nudge(rng), y), (x, y + len))
        };
        add_wire(&mut sheet, a, b);
        // A stub ending on this wire's interior, or just off it.
        if rng.next(2) == 0 {
            let mid = (
                (a.0 + b.0) / 2.0 + nudge(rng),
                (a.1 + b.1) / 2.0 + nudge(rng),
            );
            let out = if a.1 == b.1 {
                (mid.0, mid.1 - 5.0)
            } else {
                (mid.0 + 5.0, mid.1)
            };
            add_wire(&mut sheet, out, mid);
            if rng.next(2) == 0 {
                add_dot(
                    &mut sheet,
                    (mid.0 + nudge(rng), mid.1 + nudge(rng)),
                    rng.next(4) != 0,
                );
            }
        }
        // A star: two more wires ending at this wire's start.
        if rng.next(5) == 0 {
            add_wire(&mut sheet, (a.0 - 5.0, a.1), (a.0 + nudge(rng), a.1));
            add_wire(&mut sheet, (a.0, a.1 - 5.0), (a.0, a.1 + nudge(rng)));
            add_dot(&mut sheet, a, true);
        }
    }
    for _ in 0..rng.next(4) {
        let (x, y) = lattice(rng);
        add_dot(
            &mut sheet,
            (x + nudge(rng), y + nudge(rng)),
            rng.next(3) != 0,
        );
    }
    sheet
}

#[test]
fn every_reconciling_command_commits_the_full_scan_on_random_sheets() {
    let mut floors = Floors::default();
    for seed in 0..500u64 {
        let mut rng = Lcg(seed + 52_711);
        if let Some(case) = random_case(&mut rng) {
            every_command(&mut rng, &case.sheet, 5.0, &mut floors);
        }
        let rig = random_rig(&mut rng);
        every_command(&mut rng, &rig.sheet, 5.0, &mut floors);
    }
    eprintln!("random sheets: {floors:?}");
    let every = [
        "drop",
        "move",
        "rotate",
        "mirror",
        "transform",
        "delete",
        "place wire",
        "break wire",
    ];
    for name in every {
        let (cases, dots, narrowed) = floors.of(name);
        assert!(cases >= 900, "{name}: {floors:?}");
        assert!(dots >= 100, "{name}: {floors:?}");
        assert!(narrowed >= 300, "{name}: {floors:?}");
    }
}

#[test]
fn every_reconciling_command_commits_the_full_scan_near_the_tolerance() {
    let mut floors = Floors::default();
    for seed in 0..600u64 {
        let mut rng = Lcg(seed + 8_087);
        let sheet = near_tolerance_sheet(&mut rng);
        every_command(&mut rng, &sheet, 5.0, &mut floors);
    }
    eprintln!("near the tolerance: {floors:?}");
    // No parts: rotate and mirror have nothing to turn, the transform turns
    // the wires.
    for name in [
        "drop",
        "move",
        "transform",
        "delete",
        "place wire",
        "break wire",
    ] {
        let (cases, dots, narrowed) = floors.of(name);
        assert!(cases >= 550, "{name}: {floors:?}");
        assert!(dots >= 400, "{name}: {floors:?}");
        assert!(narrowed >= 400, "{name}: {floors:?}");
    }
}

#[test]
fn every_reconciling_command_commits_the_full_scan_on_the_corpus() {
    let paths = corpus_sheets();
    assert!(paths.len() >= 30, "corpus not found: {}", paths.len());
    let mut floors = Floors::default();
    let mut with_wires = 0;
    for path in &paths {
        let Ok(engine) = Engine::open(path) else {
            continue;
        };
        let sheet = engine.document().clone();
        if sheet.wires.is_empty() {
            continue;
        }
        with_wires += 1;
        let mut rng = Lcg(sheet.wires.len() as u64 + 77);
        for _ in 0..6 {
            every_command(&mut rng, &sheet, 2.54, &mut floors);
        }
    }
    eprintln!("corpus: {with_wires} sheets with wires, {floors:?}");
    assert!(
        with_wires >= 3,
        "only {with_wires} corpus sheets have wires"
    );
    let (cases, dots, narrowed) = floors.total();
    assert!(cases >= 300, "{floors:?}");
    assert!(dots >= 50, "{floors:?}");
    assert!(narrowed >= 100, "{floors:?}");
}
