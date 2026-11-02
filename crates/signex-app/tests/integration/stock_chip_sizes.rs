//! #1938 — the stock library ships chip passives in 0201 to 2512, and the
//! 0805 rows list the other sizes as alternate footprints.

use std::collections::BTreeSet;
use std::path::PathBuf;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::footprint::{Footprint, FpGraphicKind};

/// `(size, pad length, pad width, centre-to-centre)` in mm. Spelled out
/// here, apart from the generator's table, so editing one fails the test.
const LANDS: &[(&str, f64, f64, f64)] = &[
    ("0201", 0.35, 0.30, 0.70),
    ("0402", 0.55, 0.55, 1.05),
    ("0603", 0.80, 0.90, 1.65),
    ("0805", 1.00, 1.25, 2.00),
    ("1206", 1.00, 1.75, 3.05),
    ("1210", 1.00, 2.65, 3.05),
    ("2512", 1.10, 3.40, 6.20),
];

fn open_stock() -> LocalGitAdapter {
    let file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib");
    LocalGitAdapter::open(file).expect("Stock.snxlib opens")
}

fn footprint_named(adapter: &LocalGitAdapter, name: &str) -> Footprint {
    let summary = adapter
        .list_footprints()
        .expect("list footprints")
        .into_iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} is not in the stock library"));
    adapter
        .get_footprint(summary.uuid)
        .expect("footprint loads")
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn courtyard_half_width(fp: &Footprint) -> f64 {
    fp.courtyard
        .points
        .iter()
        .map(|p| p[0].abs())
        .fold(0.0, f64::max)
}

#[test]
fn every_chip_footprint_has_two_smd_pads_with_the_tabled_land() {
    let adapter = open_stock();
    for (size, len, wid, centre) in LANDS {
        for suffix in ["", "-Polarised"] {
            let name = format!("Chip-{size}{suffix}");
            let fp = footprint_named(&adapter, &name);
            assert_eq!(fp.pads.len(), 2, "{name}");
            let pad = |n: &str| fp.pads.iter().find(|p| p.number == n).expect(n);
            let (p1, p2) = (pad("1"), pad("2"));
            for p in [p1, p2] {
                assert!(p.drill.is_none(), "{name} pad {} is SMD", p.number);
                assert!(
                    close(p.size[0], *len) && close(p.size[1], *wid),
                    "{name} pad {}: {:?}",
                    p.number,
                    p.size
                );
            }
            assert!(
                close(p1.position[0], -centre / 2.0) && close(p2.position[0], centre / 2.0),
                "{name}: pads at {:?} / {:?}",
                p1.position,
                p2.position
            );
            if suffix.is_empty() && *size == "0805" {
                continue; // shipped without courtyard or fab outline
            }
            assert_eq!(fp.courtyard.points.len(), 4, "{name} courtyard");
            assert!(
                courtyard_half_width(&fp) > centre / 2.0 + len / 2.0,
                "{name} courtyard holds the copper"
            );
        }
    }
}

#[test]
fn polarised_chip_footprints_mark_pad_1_with_a_silk_bar() {
    let adapter = open_stock();
    for (size, ..) in LANDS {
        let name = format!("Chip-{size}-Polarised");
        let fp = footprint_named(&adapter, &name);
        assert_eq!(fp.silk_f.len(), 1, "{name} has one cathode bar");
        let pad_1 = fp.pads.iter().find(|p| p.number == "1").expect("pad 1");
        let FpGraphicKind::Line { from, .. } = &fp.silk_f[0].kind else {
            panic!("{name}: the cathode mark is a line");
        };
        assert!(
            from[0] < pad_1.position[0],
            "{name}: bar is on pad 1's side"
        );
        assert!(
            courtyard_half_width(&fp) > from[0].abs(),
            "{name}: courtyard holds the bar"
        );
    }
}

#[test]
fn chip_0805_rows_list_the_other_sizes_as_alternates() {
    let adapter = open_stock();
    let mut checked = 0;
    for (_, row) in adapter.iter_rows().expect("rows") {
        let pn = row.internal_pn.as_str().to_string();
        let Some(default) = &row.footprint_ref else {
            assert!(row.alternate_footprints.is_empty(), "{pn}");
            continue;
        };
        let default_name = adapter.get_footprint(default.uuid).expect("default").name;
        let suffix = match default_name.as_str() {
            "Chip-0805" => "",
            "Chip-0805-Polarised" => "-Polarised",
            _ => {
                assert!(row.alternate_footprints.is_empty(), "{pn}");
                continue;
            }
        };
        let symbol = adapter.get_symbol(row.symbol_ref.uuid).expect("symbol");
        let pins: BTreeSet<_> = symbol.pins.iter().map(|p| p.number.clone()).collect();
        let want: Vec<String> = ["0201", "0402", "0603", "1206", "1210", "2512"]
            .iter()
            .map(|s| format!("Chip-{s}{suffix}"))
            .collect();
        let got: Vec<Footprint> = row
            .alternate_footprints
            .iter()
            .map(|b| {
                adapter
                    .get_footprint(b.footprint.uuid)
                    .expect("alternate resolves")
            })
            .collect();
        let names: Vec<_> = got.iter().map(|f| f.name.clone()).collect();
        assert_eq!(names, want, "{pn}");
        for (fp, b) in got.iter().zip(&row.alternate_footprints) {
            let pads: BTreeSet<_> = fp.pads.iter().map(|p| p.number.clone()).collect();
            assert_eq!(pads, pins, "{pn} / {}", fp.name);
            assert!(b.pin_map_overrides.is_empty(), "{pn}: pin map stays 1:1");
        }
        checked += 1;
    }
    assert_eq!(
        checked, 9,
        "R, C, L, Ferrite, Fuse, D, D_Schottky, D_Zener, LED"
    );
}
