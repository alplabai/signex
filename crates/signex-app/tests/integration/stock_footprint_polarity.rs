//! #1657 — a diode or LED must not be placeable on a symmetric land with
//! no polarity mark. Pin 1 of the stock diode symbols is the cathode (K),
//! so the footprint they pair with must put the cathode on pad 1 and mark
//! that end on the silkscreen.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::footprint::{Footprint, FpGraphicKind};
use signex_library::{ComponentRow, validate_row_for_write};
use signex_sketch::entity::EntityKind;
use signex_sketch::parameter;
use signex_sketch::solver::Solver;

const GENERIC: &str = "Chip-0805";
const POLARISED: &str = "Chip-0805-Polarised";

fn open_stock() -> LocalGitAdapter {
    let file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib");
    LocalGitAdapter::open(file).expect("Stock.snxlib opens")
}

/// Rows paired with a footprint, as `(symbol cathode on pin 1, footprint)`.
fn paired_rows(adapter: &LocalGitAdapter) -> Vec<(ComponentRow, bool, Footprint)> {
    let mut out = Vec::new();
    for (_, row) in adapter.iter_rows().expect("rows") {
        let Some(fp_ref) = &row.footprint_ref else {
            continue;
        };
        let symbol = adapter.get_symbol(row.symbol_ref.uuid).expect("symbol");
        let cathode_on_1 = symbol.pins.iter().any(|p| p.number == "1" && p.name == "K");
        let footprint = adapter.get_footprint(fp_ref.uuid).expect("footprint");
        out.push((row, cathode_on_1, footprint));
    }
    assert!(
        !out.is_empty(),
        "no paired rows: the checks would be vacuous"
    );
    out
}

#[test]
fn cathode_parts_use_the_polarised_footprint_and_symmetric_parts_the_generic_one() {
    let rows = paired_rows(&open_stock());
    let mut cathode_rows = 0;
    for (row, cathode_on_1, fp) in &rows {
        let pn = row.internal_pn.as_str();
        if *cathode_on_1 {
            cathode_rows += 1;
            assert_eq!(fp.name, POLARISED, "{pn}");
        } else if fp.name.starts_with("Chip-") {
            assert_eq!(fp.name, GENERIC, "{pn}");
        }
    }
    assert_eq!(cathode_rows, 4, "D, D_Schottky, D_Zener and LED");
    for pn in [
        "STOCK-R",
        "STOCK-C",
        "STOCK-L",
        "STOCK-Ferrite",
        "STOCK-Fuse",
    ] {
        let (_, _, fp) = rows
            .iter()
            .find(|(r, _, _)| r.internal_pn.as_str() == pn)
            .unwrap_or_else(|| panic!("{pn} is not paired"));
        assert_eq!(fp.name, GENERIC, "{pn}");
    }
}

#[test]
fn descriptions_name_the_footprint_the_row_really_uses() {
    for (row, _, fp) in paired_rows(&open_stock()) {
        let pn = row.internal_pn.as_str();
        assert!(
            row.description
                .ends_with(&format!(" {} footprint.", fp.name)),
            "{pn}: {:?}",
            row.description
        );
        assert!(
            !row.description.contains("R0805"),
            "{pn}: {:?}",
            row.description
        );
    }
}

#[test]
fn polarised_footprint_pad_1_is_the_cathode_and_carries_the_mark() {
    let adapter = open_stock();
    let (_, _, fp) = paired_rows(&adapter)
        .into_iter()
        .find(|(_, cathode, _)| *cathode)
        .expect("a cathode row");
    let sketch = fp.sketch.as_ref().expect("authored sketch");
    let resolved = parameter::resolve(&sketch.parameters).expect("parameters resolve");
    let solved = Solver::default().solve(sketch, &resolved).expect("solves");
    let params: HashMap<String, f64> = resolved;

    let mut pads = Vec::new();
    signex_bake::bake_pads(sketch, &solved, &params, &mut pads, &mut Vec::new())
        .expect("pads bake");
    let x_of = |n: &str| pads.iter().find(|p| p.number == n).expect(n).position[0];
    assert_eq!(pads.len(), 2);
    assert!(x_of("1") < 0.0 && x_of("2") > 0.0);

    let (mut silk_f, mut silk_b) = (Vec::new(), Vec::new());
    signex_bake::bake_silk(sketch, &solved, &mut silk_f, &mut silk_b, &mut Vec::new())
        .expect("silk bakes");
    assert!(silk_b.is_empty());
    let [bar] = silk_f.as_slice() else {
        panic!("exactly one cathode bar, got {}", silk_f.len());
    };
    let FpGraphicKind::Line { from, to } = &bar.kind else {
        panic!("the cathode mark is a line");
    };
    // A vertical bar on the pad-1 side, outboard of pad 1's copper.
    assert_eq!(from[0], to[0]);
    assert!(
        from[0] < x_of("1") - 0.5,
        "bar sits beyond pad 1's outer edge"
    );

    assert_eq!(fp.courtyard.points.len(), 4);
    assert!(
        fp.courtyard.points.iter().any(|p| p[0] < from[0]),
        "courtyard holds the mark"
    );
    assert_eq!(fp.fab_f.len(), 2, "fab body outline plus the cathode line");

    // The sketch authored no extra pad: numbering stays 1/2.
    assert_eq!(
        sketch
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Point { .. }) && e.pad.is_some())
            .count(),
        2
    );
}

#[test]
fn the_generic_chip_footprint_has_no_mark() {
    let adapter = open_stock();
    let (_, _, fp) = paired_rows(&adapter)
        .into_iter()
        .find(|(r, _, _)| r.internal_pn.as_str() == "STOCK-R")
        .expect("STOCK-R");
    let sketch = fp.sketch.as_ref().expect("sketch");
    assert!(
        sketch
            .entities
            .iter()
            .all(|e| e.silk.is_none() && e.courtyard.is_none())
    );
}

/// Stock parts ship with an empty MPN on purpose: they are generic parts,
/// not orderable ones. Nothing in the library write path may demand one.
/// (The BOM "missing MPN" rule is a per-placed-component warning, which is
/// the right signal for a generic part the user still has to source.)
#[test]
fn stock_rows_with_an_empty_mpn_pass_write_validation() {
    for (_, row) in open_stock().iter_rows().expect("rows") {
        assert!(row.primary_mpn.mpn.is_empty(), "{}", row.internal_pn);
        validate_row_for_write(&row)
            .unwrap_or_else(|e| panic!("{}: empty MPN refused: {e}", row.internal_pn));
    }
}
