//! A symbol drag's net guard compares only the nets the move reaches
//! (the reference derives incrementally too, `derive_at` from the base, with the
//! inline self-check off so only this differential can tell them apart)
//! (`transform/rubber_band/guard_local.rs`). These tests hold it to the guard
//! that compares every net (`full_scan_guard`): the sheet a ghost step leaves,
//! the execute result and its patch, the warnings and unkept pins, the lock's
//! held references and every decision the guard took, round by round, must be
//! equal, exactly.
//!
//! The cases are seeded random sheets (symbols with wired pin tips, power
//! ports, child sheets, net / global / power labels of shared names, dots,
//! no-connects, co-selected labels), larger random grids whose far-apart
//! labels join nets by name, drags of one symbol and of many, and every sheet
//! of the corpus and the sample project. A floor on each count keeps a test
//! that stops exercising the local comparison from passing.

use std::collections::HashSet;

use crate::move_fold_local_tests::{corpus_sheets, exact};
use crate::rubber_band_tests::{Lcg, add_label, add_wire, lib_pin, lib_symbol, place, random_case};
use crate::test_support::test_sheet;
use crate::transform::rubber_band::{self_check, tally};
use crate::{CommandResult, Engine, MovePreview};
use signex_types::schematic::{
    Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind, SymbolTransform,
};
use uuid::Uuid;

type Moves = Vec<(SelectedItem, f64, f64)>;

/// Everything a ghost step leaves behind that a user or a caller can see.
#[derive(Debug, PartialEq)]
struct Outcome {
    sheet: String,
    held: Vec<String>,
    result: Option<Result<CommandResult, String>>,
    warnings: Vec<String>,
    unkept_pins: Vec<String>,
    /// Each guard check's answer, in order: whether the nets were as asked,
    /// and the tips blamed when not.
    decisions: Vec<(bool, Option<Vec<signex_net::uf::Key>>)>,
}

/// What the cases exercised.
#[derive(Debug, Default)]
struct Floors {
    cases: usize,
    local_guards: usize,
    local_checks: usize,
    full_checks: usize,
    blamed: usize,
    multi_symbol: usize,
    nets_changed: usize,
    warned: usize,
    named_nets_moved: usize,
    /// Cases where the pass rerouted, added or dropped a wire.
    rewired: usize,
}

impl Floors {
    fn report(&self, what: &str) {
        eprintln!("{what}: {self:?}");
    }
}

fn run(
    preview: &MovePreview,
    original: &SchematicSheet,
    moves: &Moves,
) -> (Outcome, tally::Tally, bool, bool) {
    let _ = tally::take();
    let moved = preview.moved_report(moves, true);
    let counts = tally::take();
    let (engine, held, result) = moved.expect("an engine");
    let outcome = Outcome {
        sheet: exact(engine.document(), original),
        held,
        result: result.map(|r| r.map_err(|e| e.to_string())),
        warnings: engine.warnings.clone(),
        unkept_pins: engine.unkept_pins.clone(),
        decisions: counts.decisions.clone(),
    };
    let changed =
        signex_net::net_signatures(engine.document()) != signex_net::net_signatures(original);
    let rewired = engine.document().wires != original.wires;
    (outcome, counts, changed, rewired)
}

/// One ghost step both ways from two previews of `sheet`.
fn step_both_ways(
    local: &MovePreview,
    full: &MovePreview,
    sheet: &SchematicSheet,
    moves: &Moves,
    what: &str,
    floors: &mut Floors,
) {
    // The differential alone must tell the two apart: no inline cross-check.
    let _off = self_check::off();
    let (a, counts, changed, rewired) = run(local, sheet, moves);
    let (b, full_counts, _, _) = run(full, sheet, moves);
    assert_eq!(a, b, "{what}");
    assert_eq!(full_counts.local_checks, 0, "{what}");
    floors.cases += 1;
    floors.local_guards += counts.guards_built_local;
    floors.local_checks += counts.local_checks;
    floors.full_checks += counts.full_checks;
    floors.blamed += usize::from(counts.blames > 0 && counts.local_checks > 0);
    floors.nets_changed += usize::from(changed && counts.local_checks > 0);
    floors.warned += usize::from(!a.warnings.is_empty());
    floors.rewired += usize::from(rewired && counts.local_checks > 0);
    let symbols = (moves.iter())
        .filter(|(item, _, _)| item.kind == SelectedKind::Symbol)
        .count();
    floors.multi_symbol += usize::from(symbols >= 2 && counts.local_checks > 0);
    let moved: HashSet<Uuid> = moves.iter().map(|(item, _, _)| item.uuid).collect();
    let named = signex_net::net_signatures(sheet)
        .iter()
        .any(|n| !n.name.is_empty() && n.terminals.iter().any(|(s, _)| moved.contains(s)));
    floors.named_nets_moved += usize::from(named && counts.local_checks > 0);
}

fn previews(sheet: &SchematicSheet) -> (MovePreview, MovePreview) {
    let local = MovePreview::new(sheet);
    let mut full = MovePreview::new(sheet);
    // The reference compares every net and asks every question of the sheet.
    full.full_scan_guard = true;
    full.full_scan_follow = true;
    (local, full)
}

#[test]
fn the_local_guard_equals_the_full_guard_on_random_sheets() {
    let mut floors = Floors::default();
    for seed in 0..1500u64 {
        let mut rng = Lcg(seed + 40_009);
        let Some(case) = random_case(&mut rng) else {
            continue;
        };
        let (local, full) = previews(&case.sheet);
        let what = format!("seed {seed} moves {:?}", case.moves);
        step_both_ways(&local, &full, &case.sheet, &case.moves, &what, &mut floors);
    }
    floors.report("random sheets");
    assert!(floors.cases >= 1200, "{floors:?}");
    assert!(floors.local_guards >= 600, "{floors:?}");
    assert!(floors.local_checks >= 900, "{floors:?}");
    assert!(floors.blamed >= 50, "{floors:?}");
    assert!(floors.nets_changed >= 150, "{floors:?}");
    assert!(floors.multi_symbol >= 300, "{floors:?}");
    assert!(floors.warned >= 20, "{floors:?}");
    assert!(floors.rewired >= 600, "{floors:?}");
    assert!(floors.named_nets_moved >= 200, "{floors:?}");
}

/// A grid of cells: a part with wires on its pins, labels at the far ends
/// drawn from a small pool of names (so cells far apart join nets by name),
/// the odd power port, dot and no-connect.
fn grid_sheet(rng: &mut Lcg, cols: u64, rows: u64) -> (SchematicSheet, Vec<Uuid>) {
    let mut sheet = test_sheet();
    let mut ic = lib_symbol("IC", vec![lib_pin("1", -5.0), lib_pin("2", -5.0)]);
    ic.pins.push(ic.pins[0].clone());
    ic.pins.push(ic.pins[0].clone());
    for (i, (x, y)) in [(-5.0, -2.5), (-5.0, 2.5), (5.0, -2.5), (5.0, 2.5)]
        .into_iter()
        .enumerate()
    {
        ic.pins[i].pin.position = Point::new(x, y);
        ic.pins[i].pin.number = (i + 1).to_string();
    }
    sheet.lib_symbols.insert("IC".to_string(), ic);
    sheet.lib_symbols.insert(
        "R".to_string(),
        lib_symbol("R", vec![lib_pin("1", -5.0), lib_pin("2", 5.0)]),
    );
    sheet.lib_symbols.insert(
        "GND".to_string(),
        lib_symbol("GND", vec![lib_pin("1", 0.0)]),
    );
    let names = ["A", "B", "C", "VCC", "GND", "SDA", "SCL"];
    let kinds = [LabelType::Net, LabelType::Global, LabelType::Power];
    let mut movable = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let at = Point::new(30.0 + 50.0 * col as f64, 30.0 + 30.0 * row as f64);
            let lib = match rng.next(10) {
                0..=3 => "IC",
                4..=8 => "R",
                _ => "GND",
            };
            let reference = format!("X{row}_{col}");
            let id = place(&mut sheet, &reference, lib, at);
            let power = lib == "GND";
            let sym = sheet.symbols.last_mut().expect("placed");
            sym.rotation = 90.0 * rng.next(4) as f64;
            sym.mirror_y = rng.next(4) == 0;
            if power {
                sym.is_power = true;
                sym.value = names[rng.next(names.len() as u64) as usize].to_string();
            } else {
                movable.push(id);
            }
            let sym = sheet.symbols.last().expect("placed").clone();
            let pins = sheet.lib_symbols[lib].pins.clone();
            for pin in &pins {
                if rng.next(100) >= 75 {
                    continue;
                }
                let tip = SymbolTransform::from_symbol(&sym).apply(pin.pin.position);
                let end = match rng.next(3) {
                    0 => (tip.x + 5.0, tip.y),
                    1 => (tip.x - 5.0, tip.y),
                    _ => (tip.x, tip.y + 5.0),
                };
                add_wire(&mut sheet, (tip.x, tip.y), end);
                if rng.next(100) < 55 {
                    let text = names[rng.next(names.len() as u64) as usize];
                    let kind = kinds[rng.next(3) as usize];
                    sheet
                        .labels
                        .push(Label::new(text, Point::new(end.0, end.1), kind));
                } else if rng.next(100) < 30 {
                    add_label(&mut sheet, &format!("L{row}_{col}"), end);
                }
            }
            // A wire to the cell on the right, now and then.
            if col + 1 < cols && rng.next(100) < 25 {
                add_wire(
                    &mut sheet,
                    (at.x + 12.5, at.y + 10.0),
                    (at.x + 37.5, at.y + 10.0),
                );
            }
        }
    }
    (sheet, movable)
}

fn grid_moves(rng: &mut Lcg, sheet: &SchematicSheet, movable: &[Uuid], count: usize) -> Moves {
    let centre = sheet.symbols[rng.next(sheet.symbols.len() as u64) as usize].position;
    let mut near: Vec<(f64, Uuid)> = sheet
        .symbols
        .iter()
        .filter(|s| movable.contains(&s.uuid))
        .map(|s| {
            (
                (s.position.x - centre.x).hypot(s.position.y - centre.y),
                s.uuid,
            )
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let hop = |rng: &mut Lcg| match rng.next(6) {
        0 => 50.0,
        1 => -50.0,
        2 => 30.0,
        _ => 2.5 * (rng.next(17) as f64 - 8.0),
    };
    let (dx, dy) = (hop(rng), hop(rng));
    near.into_iter()
        .take(count)
        .map(|(_, id)| (SelectedItem::new(id, SelectedKind::Symbol), dx, dy))
        .collect()
}

#[test]
fn the_local_guard_equals_the_full_guard_on_grids_with_far_name_joins() {
    let mut floors = Floors::default();
    for seed in 0..120u64 {
        let mut rng = Lcg(seed + 770_017);
        let (sheet, movable) = grid_sheet(&mut rng, 8, 6);
        let (local, full) = previews(&sheet);
        for count in [1usize, 2, 5, 10, 24] {
            for _ in 0..2 {
                let moves = grid_moves(&mut rng, &sheet, &movable, count);
                let what = format!("grid seed {seed} count {count} moves {moves:?}");
                step_both_ways(&local, &full, &sheet, &moves, &what, &mut floors);
            }
        }
    }
    floors.report("grids");
    assert!(floors.cases >= 1000, "{floors:?}");
    assert!(floors.local_guards >= 700, "{floors:?}");
    assert!(floors.local_checks >= 1000, "{floors:?}");
    assert!(floors.blamed >= 100, "{floors:?}");
    assert!(floors.nets_changed >= 150, "{floors:?}");
    assert!(floors.multi_symbol >= 700, "{floors:?}");
    assert!(floors.named_nets_moved >= 600, "{floors:?}");
    assert!(floors.rewired >= 700, "{floors:?}");
}

/// A drag: one preview serves a run of cursor steps over a growing delta.
#[test]
fn the_local_guard_equals_the_full_guard_over_a_drag() {
    let mut floors = Floors::default();
    for seed in 0..40u64 {
        let mut rng = Lcg(seed + 31_337);
        let (sheet, movable) = grid_sheet(&mut rng, 8, 6);
        let (local, full) = previews(&sheet);
        for count in [1usize, 10] {
            let base = grid_moves(&mut rng, &sheet, &movable, count);
            for step in 1..=10 {
                let moves: Moves = base
                    .iter()
                    .map(|(item, _, _)| (*item, 2.5 * step as f64, 2.5 * (step % 4) as f64))
                    .collect();
                let what = format!("drag seed {seed} count {count} step {step}");
                step_both_ways(&local, &full, &sheet, &moves, &what, &mut floors);
            }
        }
    }
    floors.report("drags");
    assert!(floors.cases >= 800, "{floors:?}");
    assert!(floors.local_checks >= 800, "{floors:?}");
    assert!(floors.multi_symbol >= 350, "{floors:?}");
}

#[test]
fn the_local_guard_equals_the_full_guard_on_the_corpus_and_the_sample_project() {
    let paths = corpus_sheets();
    assert!(paths.len() >= 30, "corpus not found: {}", paths.len());
    let mut floors = Floors::default();
    let (mut with_symbols, mut sample_sheets) = (0, 0);
    for path in &paths {
        let Ok(engine) = Engine::open(path) else {
            continue;
        };
        let sheet = engine.document().clone();
        if sheet.symbols.is_empty() {
            continue;
        }
        with_symbols += 1;
        sample_sheets += usize::from(path.to_string_lossy().contains("sample-project"));
        let (local, full) = previews(&sheet);
        let mut rng = Lcg(sheet.symbols.len() as u64 + sheet.wires.len() as u64 + 5);
        for count in [1usize, 3, 10, 50] {
            for trial in 0..4 {
                let count = count.min(sheet.symbols.len());
                let mut picked: Vec<Uuid> = Vec::new();
                while picked.len() < count {
                    let id = sheet.symbols[rng.next(sheet.symbols.len() as u64) as usize].uuid;
                    if !picked.contains(&id) {
                        picked.push(id);
                    }
                }
                let dx = 2.54 * (rng.next(9) as f64 - 4.0);
                let dy = 2.54 * (rng.next(9) as f64 - 4.0);
                let moves: Moves = picked
                    .into_iter()
                    .map(|id| (SelectedItem::new(id, SelectedKind::Symbol), dx, dy))
                    .collect();
                let what = format!("{} count {count} trial {trial}", path.display());
                step_both_ways(&local, &full, &sheet, &moves, &what, &mut floors);
            }
        }
    }
    floors.report("corpus");
    eprintln!("corpus: {with_symbols} sheets with symbols, {sample_sheets} of the sample project");
    assert!(
        with_symbols >= 8,
        "only {with_symbols} corpus sheets have symbols"
    );
    assert!(sample_sheets >= 1, "the sample project was not found");
    assert!(floors.cases >= 100, "{floors:?}");
    assert!(floors.local_guards >= 15, "{floors:?}");
    assert!(floors.local_checks >= 20, "{floors:?}");
    assert!(floors.blamed >= 1, "{floors:?}");
}

/// A move with a power port or a child-sheet pin among its tips is compared
/// over every net: no local guard is built for it.
#[test]
fn a_power_port_or_a_child_sheet_pin_is_compared_over_every_net() {
    let (mut power, mut child) = (0, 0);
    for seed in 0..400u64 {
        let mut rng = Lcg(seed + 9_001);
        let Some(case) = random_case(&mut rng) else {
            continue;
        };
        let has_power = case.moves.iter().any(|(item, _, _)| {
            case.sheet
                .symbols
                .iter()
                .any(|s| s.uuid == item.uuid && s.is_power)
        });
        let has_child = case.moves.iter().any(|(item, _, _)| {
            matches!(item.kind, SelectedKind::ChildSheet | SelectedKind::SheetPin)
        });
        if !has_power && !has_child {
            continue;
        }
        let (local, _) = previews(&case.sheet);
        let _ = tally::take();
        let _ = local.moved_report(&case.moves, true);
        let counts = tally::take();
        assert_eq!(counts.guards_built_local, 0, "seed {seed}");
        power += usize::from(has_power);
        child += usize::from(has_child);
    }
    assert!(power >= 20 && child >= 20, "{power} power, {child} child");
}

/// A net the plain move did not reach, which the candidate touches and leaves
/// as it was (a far wire split in two with a new id), is no difference: the
/// local comparison must restore it on the intent's side, as the comparison
/// over every net does by comparing every net.
#[test]
fn a_far_net_the_candidate_touches_and_leaves_as_it_was_is_no_difference() {
    use crate::rubber_band_tests::add_resistor;
    use crate::{Guard, Policy, follow_moved_pins, moved_pin_tips};
    use std::sync::Arc;

    let mut before = test_sheet();
    let r1 = add_resistor(&mut before, "R1", Point::new(50.0, 50.0));
    let r2 = add_resistor(&mut before, "R2", Point::new(150.0, 50.0));
    add_wire(&mut before, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut before, (130.0, 50.0), (145.0, 50.0));
    let items = [SelectedItem::new(r1, SelectedKind::Symbol)];
    let tips = moved_pin_tips(&before, &items);
    let mut plain = before.clone();
    plain
        .symbols
        .iter_mut()
        .find(|s| s.uuid == r1)
        .unwrap()
        .position
        .y += 10.0;
    let mut candidate = plain.clone();
    let follow = follow_moved_pins(&mut candidate, &tips, &items, &Policy::default());
    // The far net (R2's) is split at its middle, the halves as good as one.
    let far = candidate
        .wires
        .iter()
        .position(|w| w.start.x == 130.0)
        .expect("the far wire");
    let mut tail = candidate.wires[far].clone();
    tail.uuid = Uuid::now_v7();
    tail.start = Point::new(137.5, 50.0);
    candidate.wires[far].end = Point::new(137.5, 50.0);
    candidate.wires.push(tail);
    assert!(before.symbols.iter().any(|s| s.uuid == r2));

    let base = Arc::new(signex_net::NetBase::new(&before));
    let nets = Arc::new(base.derive(&before));
    let policy = Policy::default();
    let guard = || {
        Guard::with_base(
            &before,
            &plain,
            &tips,
            Some(nets.clone()),
            Some(base.clone()),
        )
    };
    let _ = tally::take();
    let local = guard().check(&candidate, &tips, &policy, &follow.edited_tips);
    let local_counts = tally::take();
    let full = guard()
        .without_local()
        .check(&candidate, &tips, &policy, &follow.edited_tips);
    let full_counts = tally::take();
    assert_eq!(local_counts.local_checks, 1, "the comparison was not local");
    assert_eq!(local, full);
    assert_eq!(local_counts.decisions, full_counts.decisions);
    assert!(local_counts.decisions[0].0, "the nets are as asked");
}

/// A pin with two wires on it, the second of which cannot follow (a label
/// sits on its route): the first wire's move is undone and the pin gets a
/// new path. The rollback is held to the whole-list copy it replaced.
#[test]
fn a_follow_that_fails_on_its_second_wire_puts_the_first_back() {
    use crate::rubber_band_tests::add_resistor;
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (45.0, 30.0), (45.0, 50.0));
    add_label(&mut sheet, "X", (45.0, 55.0));
    let (local, full) = previews(&sheet);
    let mut floors = Floors::default();
    for (dx, dy) in [(0.0, 10.0), (0.0, 7.5), (2.5, 10.0), (-5.0, 10.0)] {
        let moves: Moves = vec![(SelectedItem::new(r1, SelectedKind::Symbol), dx, dy)];
        let what = format!("rollback {dx},{dy}");
        step_both_ways(&local, &full, &sheet, &moves, &what, &mut floors);
    }
    assert!(
        floors.local_checks >= 4 && floors.rewired >= 1,
        "{floors:?}"
    );
}
