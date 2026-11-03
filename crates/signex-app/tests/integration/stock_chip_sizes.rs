//! #1938 — the stock library ships chip passives in 0201 to 2512, and the
//! 0805 rows list the other sizes as alternate footprints.

use std::collections::BTreeSet;
use std::path::PathBuf;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::footprint::{Footprint, FpGraphicKind};

/// `(size, pad length, pad width, centre-to-centre, Z, G)` in mm. Spelled
/// out here, apart from the generator's table, so editing one fails the
/// test. Z is the outer span and G the inner gap.
const LANDS: &[(&str, f64, f64, f64, f64, f64)] = &[
    ("0201", 0.46, 0.42, 0.66, 1.12, 0.20),
    ("0402", 0.62, 0.62, 0.90, 1.52, 0.28),
    ("0603", 0.95, 1.00, 1.60, 2.55, 0.65),
    ("0805", 1.15, 1.45, 1.80, 2.95, 0.65),
    ("1206", 1.15, 1.80, 3.00, 4.15, 1.85),
    ("1210", 1.15, 2.70, 3.00, 4.15, 1.85),
    ("2512", 1.45, 3.50, 6.20, 7.65, 4.75),
];

/// `(size, body length, body width, courtyard length, courtyard width,
/// polarised courtyard length, max height)` in mm. Courtyards are the KEMET
/// C1003 Table 3 Level B V1 x V2 (2512: pad extent + 0.25 per side, rounded
/// up to 0.1); the polarised one holds the cathode bar + 0.2, rounded up to
/// 0.1. Heights are the thickest KEMET Table 2A code (2512: Vishay H max).
const OUTLINES: &[(&str, f64, f64, f64, f64, f64, f64)] = &[
    ("0201", 0.6, 0.3, 1.5, 0.8, 2.0, 0.33),
    ("0402", 1.0, 0.5, 1.9, 1.0, 2.4, 0.60),
    ("0603", 1.6, 0.8, 3.1, 1.5, 3.4, 0.92),
    ("0805", 2.0, 1.25, 3.5, 2.0, 3.8, 1.40),
    ("1206", 3.2, 1.6, 4.7, 2.3, 5.0, 1.80),
    ("1210", 3.2, 2.5, 4.7, 3.2, 5.0, 2.30),
    ("2512", 6.4, 3.2, 8.2, 4.0, 8.5, 0.70),
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
    for (size, len, wid, centre, z, g) in LANDS {
        assert!(
            close((z - g) / 2.0, *len) && close((z + g) / 2.0, *centre),
            "{size}: Z/G"
        );
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

#[test]
fn polarised_bar_clears_the_copper_and_its_mask_opening() {
    let adapter = open_stock();
    for (size, ..) in LANDS {
        let name = format!("Chip-{size}-Polarised");
        let fp = footprint_named(&adapter, &name);
        let pad_1 = fp.pads.iter().find(|p| p.number == "1").expect("pad 1");
        let mask = pad_1.solder_mask_margin.unwrap_or(0.0);
        let copper_edge = pad_1.position[0].abs() + pad_1.size[0] / 2.0;
        let bar = &fp.silk_f[0];
        let FpGraphicKind::Line { from, .. } = &bar.kind else {
            panic!("{name}: the cathode mark is a line");
        };
        let inner_edge = from[0].abs() - bar.stroke_width / 2.0;
        assert!(
            inner_edge >= copper_edge + mask + 0.1 - 1e-9,
            "{name}: bar inner edge {inner_edge} vs copper {copper_edge} + mask {mask}"
        );
    }
}

#[test]
fn chip_outlines_courtyards_and_heights_are_the_tabled_ones() {
    let adapter = open_stock();
    for (size, bl, bw, cl, cw, pcl, height) in OUTLINES {
        for (suffix, court_len) in [("", cl), ("-Polarised", pcl)] {
            let name = format!("Chip-{size}{suffix}");
            let fp = footprint_named(&adapter, &name);
            let xs = || fp.courtyard.points.iter().map(|p| p[0].abs());
            let ys = || fp.courtyard.points.iter().map(|p| p[1].abs());
            assert!(
                xs().all(|x| close(x, court_len / 2.0)) && ys().all(|y| close(y, cw / 2.0)),
                "{name}: courtyard {:?}",
                fp.courtyard.points
            );
            let FpGraphicKind::Rectangle { from, to } = &fp.fab_f[0].kind else {
                panic!("{name}: the fab body is a rectangle");
            };
            assert!(
                close(to[0] - from[0], *bl) && close(to[1] - from[1], *bw),
                "{name}: fab body {from:?} {to:?}"
            );
            assert!(
                (fp.body_3d.height_mm - *height as f32).abs() < 1e-6,
                "{name}: height {}",
                fp.body_3d.height_mm
            );
        }
    }
}

/// #1938 — Stock is Personal mode and applies a newer row version silently
/// on open, so the chip lands must not move any version: existing designs
/// open clean and unchanged.
#[test]
fn no_stock_footprint_or_row_version_was_bumped() {
    let adapter = open_stock();
    for summary in adapter.list_footprints().expect("list footprints") {
        let fp = adapter.get_footprint(summary.uuid).expect("footprint");
        assert_eq!(fp.version, "0.0.1", "{}", fp.name);
    }
    for (_, row) in adapter.iter_rows().expect("rows") {
        let pn = row.internal_pn.as_str().to_string();
        assert_eq!(row.version, "0.0.1", "{pn}");
        if let Some(default) = &row.footprint_ref {
            assert_eq!(row.footprint_version, "0.0.1", "{pn}");
            let fp = adapter.get_footprint(default.uuid).expect("default");
            assert_eq!(row.footprint_version, fp.version, "{pn}");
        }
    }
}
