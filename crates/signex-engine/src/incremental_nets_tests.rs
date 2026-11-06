//! The drag ghost derives nets from the pre-move connectivity instead of from
//! scratch (#2077). Every such derive inside the guard is already held to the
//! full derive (see `guard::derive_at`); these tests drive whole moves through
//! it and hold the ghost to the commit.

use crate::rubber_band_tests::{
    Case, Lcg, add_label, add_resistor, add_wire, child_fixture, describe, lib_pin, lib_symbol,
    place, random_case, series_fixture, symbol_item,
};
use crate::test_support::test_sheet;
use crate::{Command, Engine, MovePreview};
use signex_net::{NetBase, net_signatures};
use signex_types::schematic::{Bus, BusEntry, Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

type Moves = [(SelectedItem, f64, f64)];

/// The ghost equals the commit and the move run without the incremental
/// derive, and the base derives the result as the full derive does.
fn ghost_equals_commit(sheet: &SchematicSheet, moves: &Moves) {
    let ghost = MovePreview::new(sheet).moved(moves, true);
    let full = MovePreview::new_full_derive(sheet).moved(moves, true);
    let mut engine = Engine::new(sheet.clone()).expect("engine");
    engine
        .execute(Command::MoveEach {
            moves: moves.to_vec(),
            keep_wires: true,
        })
        .expect("move");
    let shown = describe(sheet);
    assert_eq!(
        describe(&ghost),
        describe(engine.document()),
        "ghost vs commit\n{shown}"
    );
    assert_eq!(
        describe(&ghost),
        describe(&full),
        "incremental vs full ghost\n{shown}"
    );
    let base = NetBase::new(sheet);
    assert!(base.derives_incrementally(&ghost), "fell back\n{shown}");
    assert_eq!(base.derive(&ghost), net_signatures(&ghost), "{shown}");
}

#[test]
fn the_preview_starts_from_the_same_nets_as_the_full_derive() {
    let (engine, _, _) = series_fixture();
    let sheet = engine.document();
    let preview = MovePreview::new(sheet);
    assert_eq!(*preview.before, *crate::before_nets(sheet));
}

fn gnd_port(sheet: &mut SchematicSheet, reference: &str, value: &str, at: Point) -> Uuid {
    sheet
        .lib_symbols
        .entry(value.to_string())
        .or_insert_with(|| lib_symbol(value, vec![lib_pin("1", 0.0)]));
    let id = place(sheet, reference, value, at);
    let sym = sheet.symbols.iter_mut().find(|s| s.uuid == id).unwrap();
    sym.is_power = true;
    sym.value = value.to_string();
    id
}

#[test]
fn a_rubber_banded_wire_follows_in_the_ghost() {
    let (engine, r1, _) = series_fixture();
    for (dx, dy) in [(-5.0, 0.0), (0.0, 10.0), (7.5, -5.0), (40.0, 0.0)] {
        ghost_equals_commit(engine.document(), &[(symbol_item(r1), dx, dy)]);
    }
}

#[test]
fn a_t_junction_is_made_and_unmade_by_a_move() {
    let mut sheet = test_sheet();
    add_wire(&mut sheet, (30.0, 50.0), (100.0, 50.0));
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 60.0));
    add_wire(&mut sheet, (45.0, 60.0), (45.0, 50.0));
    add_wire(&mut sheet, (55.0, 60.0), (55.0, 70.0));
    for (dx, dy) in [(10.0, 0.0), (-10.0, 0.0), (0.0, -10.0), (15.0, 5.0)] {
        ghost_equals_commit(&sheet, &[(symbol_item(r1), dx, dy)]);
    }
}

#[test]
fn a_move_that_would_merge_another_net_is_refused_alike() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_resistor(&mut sheet, "R2", Point::new(80.0, 70.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (55.0, 50.0), (60.0, 50.0));
    add_wire(&mut sheet, (75.0, 70.0), (60.0, 70.0));
    add_wire(&mut sheet, (60.0, 40.0), (60.0, 80.0));
    for (dx, dy) in [(0.0, 20.0), (5.0, 0.0), (10.0, 10.0), (-10.0, 20.0)] {
        ghost_equals_commit(&sheet, &[(symbol_item(r1), dx, dy)]);
    }
}

#[test]
fn labels_that_join_nets_by_name_stay_joined_in_the_ghost() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_resistor(&mut sheet, "R2", Point::new(50.0, 80.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (30.0, 80.0), (45.0, 80.0));
    add_label(&mut sheet, "A", (35.0, 50.0));
    add_label(&mut sheet, "A", (35.0, 80.0));
    for (dx, dy) in [(0.0, 10.0), (-10.0, 0.0), (0.0, 30.0)] {
        ghost_equals_commit(&sheet, &[(symbol_item(r1), dx, dy)]);
    }
}

#[test]
fn power_ports_of_one_value_stay_one_net_in_the_ghost() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    let r2 = add_resistor(&mut sheet, "R2", Point::new(50.0, 80.0));
    let g1 = gnd_port(&mut sheet, "#PWR1", "GND", Point::new(30.0, 50.0));
    gnd_port(&mut sheet, "#PWR2", "GND", Point::new(30.0, 80.0));
    gnd_port(&mut sheet, "#PWR3", "VCC", Point::new(70.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (30.0, 80.0), (45.0, 80.0));
    add_wire(&mut sheet, (55.0, 50.0), (70.0, 50.0));
    for item in [g1, r1, r2] {
        for (dx, dy) in [(0.0, 10.0), (-10.0, 0.0), (20.0, 20.0)] {
            ghost_equals_commit(&sheet, &[(symbol_item(item), dx, dy)]);
        }
    }
}

#[test]
fn bus_entries_do_not_disturb_the_ghost() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    sheet.buses.push(Bus {
        uuid: Uuid::now_v7(),
        start: Point::new(20.0, 30.0),
        end: Point::new(20.0, 70.0),
    });
    sheet.bus_entries.push(BusEntry {
        uuid: Uuid::now_v7(),
        position: Point::new(20.0, 50.0),
        size: (10.0, 0.0),
    });
    ghost_equals_commit(&sheet, &[(symbol_item(r1), 0.0, 10.0)]);
}

#[test]
fn a_child_sheet_pin_drags_its_wire_in_the_ghost() {
    let (engine, child, pin) = child_fixture();
    let child_item = SelectedItem::new(child, SelectedKind::ChildSheet);
    ghost_equals_commit(engine.document(), &[(child_item, 10.0, 5.0)]);
    ghost_equals_commit(engine.document(), &[(child_item, -30.0, 0.0)]);
    let pin_item = SelectedItem::new(pin, SelectedKind::SheetPin);
    ghost_equals_commit(engine.document(), &[(pin_item, 0.0, 5.0)]);
}

#[test]
fn random_moves_give_the_commit_in_the_ghost() {
    let mut cases = 0;
    for seed in [0x5EED_2077, 0xC0FFEE_u64, 0xBADC_AB1E, 0x1234_5678] {
        let mut rng = Lcg(seed);
        for _ in 0..600 {
            let Some(Case { sheet, moves, .. }) = random_case(&mut rng) else {
                continue;
            };
            ghost_equals_commit(&sheet, &moves);
            cases += 1;
        }
    }
    assert!(cases > 1500, "only {cases} cases");
}

/// Timing bench: one drag step on ~480 symbols and ~1000 wires, derived from
/// scratch against derived from the pre-move connectivity.
///
/// `cargo test -p signex-engine --release incremental_nets_bench -- --ignored --nocapture`
#[test]
#[ignore = "timing bench; run with --release --ignored --nocapture"]
fn incremental_nets_bench() {
    use std::time::Instant;

    let mut sheet = test_sheet();
    let mut middle = None;
    for row in 0..20 {
        for col in 0..24 {
            let (x, y) = (20.0 + 20.0 * col as f64, 20.0 + 10.0 * row as f64);
            let id = add_resistor(
                &mut sheet,
                &format!("R{}", row * 24 + col),
                Point::new(x, y),
            );
            if (row, col) == (10, 12) {
                middle = Some(id);
            }
            add_wire(&mut sheet, (x - 5.0, y), (x - 10.0, y));
            add_wire(&mut sheet, (x + 5.0, y), (x + 10.0, y));
        }
    }
    let target = symbol_item(middle.expect("middle symbol"));
    println!(
        "sheet: {} symbols, {} wires",
        sheet.symbols.len(),
        sheet.wires.len()
    );

    let steps: Vec<(f64, f64)> = (1..=20)
        .map(|k| (2.5 * k as f64, 2.5 * (k % 4) as f64))
        .collect();
    let time = |preview: &MovePreview| -> Vec<f64> {
        steps
            .iter()
            .map(|&(dx, dy)| {
                let start = Instant::now();
                let ghost = preview.moved(&[(target, dx, dy)], true);
                let ms = start.elapsed().as_secs_f64() * 1000.0;
                std::hint::black_box(ghost);
                ms
            })
            .collect()
    };
    // The guard checks every incremental answer against the full derive in
    // tests; that would be timed here too.
    crate::transform::rubber_band::self_check::set(false);
    let started = Instant::now();
    let incremental = MovePreview::new(&sheet);
    let new_cost = started.elapsed().as_secs_f64() * 1000.0;
    let full = MovePreview::new_full_derive(&sheet);
    // One warm-up step each, then the timed run.
    time(&incremental);
    time(&full);
    let (new_ms, old_ms) = (time(&incremental), time(&full));
    let stats = |ms: &[f64]| {
        let mean = ms.iter().sum::<f64>() / ms.len() as f64;
        let min = ms.iter().copied().fold(f64::INFINITY, f64::min);
        (mean, min)
    };
    let (old_mean, old_min) = stats(&old_ms);
    let (new_mean, new_min) = stats(&new_ms);
    println!("per drag step (ms)    mean     min");
    println!("  from scratch     {old_mean:8.3} {old_min:8.3}");
    println!("  incremental      {new_mean:8.3} {new_min:8.3}");
    println!(
        "  speed-up         {:8.2}x {:7.2}x",
        old_mean / new_mean,
        old_min / new_min
    );
    println!("once per drag: incremental preview setup {new_cost:.3} ms");

    crate::transform::rubber_band::self_check::set(true);
    for &(dx, dy) in &steps {
        let moves = [(target, dx, dy)];
        assert_eq!(
            describe(&incremental.moved(&moves, true)),
            describe(&full.moved(&moves, true)),
            "step ({dx}, {dy})"
        );
    }
}
