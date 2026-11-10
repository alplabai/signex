//! Five realistic boards in `tests/corpus/pcb/real/` (the PCB Layout exit
//! criterion "five real `.snxpcb` fixtures render"), each a `.snxpcb` plus
//! the `.snxprj` that carries its net classes.
//!
//! The files are written by the deterministic generator in this module.
//! `SIGNEX_BLESS_CORPUS=1` rewrites them; without it the test asserts the
//! committed bytes are exactly what the generator writes, so the files never
//! drift from their source. The generic corpus harness (`round_trip_corpus`)
//! walks them too. Render, DRC and ratsnest checks live in `signex-renderer`
//! and `pcb-geom`. What each board covers: `tests/corpus/README.md`.

mod builder;
mod linear_regulator;
mod mcu_breakout;
mod mixed_tht_smd;
mod parts;
mod sensor_4layer;
mod usb_c_power;

use std::fs;
use std::path::{Path, PathBuf};

use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::net_class::NetClass;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::project::{ProjectData, encode_project, parse_project_bytes};
use uuid::Uuid;

use builder::Builder;

const BLESS_ENV: &str = "SIGNEX_BLESS_CORPUS";

/// One generated board: its file stem, the board and its net classes.
struct RealBoard {
    name: &'static str,
    board: PcbBoard,
    classes: Vec<NetClass>,
}

fn generate() -> Vec<RealBoard> {
    let built: [(&'static str, Builder); 5] = [
        ("linear-regulator", linear_regulator::build()),
        ("usb-c-power-input", usb_c_power::build()),
        ("mcu-breakout-tqfp32", mcu_breakout::build()),
        ("sensor-4layer", sensor_4layer::build()),
        ("mixed-tht-smd", mixed_tht_smd::build()),
    ];
    built
        .into_iter()
        .map(|(name, builder)| {
            let (board, classes) = builder.finish();
            RealBoard {
                name,
                board,
                classes,
            }
        })
        .collect()
}

fn real_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/pcb/real")
}

fn pcb_text(real: &RealBoard) -> String {
    SnxPcb::new(real.board.clone())
        .write_string()
        .unwrap_or_else(|e| panic!("{}: write failed: {e}", real.name))
}

fn project(real: &RealBoard) -> ProjectData {
    let json = serde_json::json!({
        "name": real.name,
        "dir": "",
        "schematic_root": null,
        "pcb_file": format!("{}.snxpcb", real.name),
        "net_classes": real.classes,
    });
    serde_json::from_value(json).expect("project data")
}

fn project_bytes(real: &RealBoard) -> Vec<u8> {
    encode_project(&project(real)).expect("project encodes")
}

#[test]
fn real_boards_match_their_generator() {
    let blessing = std::env::var_os(BLESS_ENV).is_some_and(|v| v == "1");
    let dir = real_dir();
    let mut stale = Vec::new();
    for real in generate() {
        let files = [
            (
                format!("{}.snxpcb", real.name),
                pcb_text(&real).into_bytes(),
            ),
            (format!("{}.snxprj", real.name), project_bytes(&real)),
        ];
        for (file, bytes) in files {
            let path = dir.join(&file);
            if blessing {
                fs::create_dir_all(&dir).unwrap();
                fs::write(&path, &bytes).unwrap();
            } else if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
                stale.push(file);
            }
        }
    }
    assert!(
        stale.is_empty(),
        "generated corpus files differ from the generator: {stale:?}\n(an intentional change: rerun with {BLESS_ENV}=1 and review the diff)"
    );
}

fn load(name: &str) -> (String, PcbBoard) {
    let text = fs::read_to_string(real_dir().join(format!("{name}.snxpcb"))).expect("file");
    let board = SnxPcb::parse(&text)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .board;
    (text, board)
}

type Counts = (
    &'static str,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
);

/// `(name, footprints, pads, tracks, vias, zones, nets, copper layers)`.
const COUNTS: [Counts; 5] = [
    ("linear-regulator", 7, 16, 16, 4, 1, 4, 2),
    ("usb-c-power-input", 9, 26, 23, 5, 1, 6, 2),
    ("mcu-breakout-tqfp32", 12, 78, 109, 9, 1, 29, 2),
    ("sensor-4layer", 8, 27, 34, 15, 2, 5, 4),
    ("mixed-tht-smd", 10, 22, 20, 1, 1, 5, 2),
];

fn counts(name: &'static str, board: &PcbBoard) -> Counts {
    (
        name,
        board.footprints.len(),
        board.footprints.iter().map(|f| f.pads.len()).sum(),
        board.segments.len(),
        board.vias.len(),
        board.zones.len(),
        board.nets.len(),
        board
            .layers
            .iter()
            .filter(|l| l.name.ends_with("Copper"))
            .count(),
    )
}

#[test]
fn real_boards_hold_the_pinned_item_counts() {
    let found: Vec<Counts> = COUNTS
        .iter()
        .map(|&(name, ..)| counts(name, &load(name).1))
        .collect();
    assert_eq!(found, COUNTS.to_vec());
}

#[test]
fn real_boards_reload_resave_identically_and_round_trip() {
    for (name, ..) in COUNTS {
        let (text, mut board) = load(name);
        let resaved = SnxPcb::new(board.clone()).write_string().unwrap();
        assert_eq!(resaved, text, "{name}: a no-edit save changes the file");
        let again = SnxPcb::parse(&resaved).unwrap().board;
        assert_eq!(
            serde_json::to_value(&again).unwrap(),
            serde_json::to_value(&board).unwrap(),
            "{name}: the board changes over a save and reload"
        );
        assert!(
            board.take_load_repairs().is_empty(),
            "{name}: load repaired"
        );
    }
}

#[test]
fn the_mixed_board_carries_its_board_sketch() {
    let (_, board) = load("mixed-tht-smd");
    let sketch = board.sketch.as_ref().expect("[board_sketch] section");
    assert_eq!(sketch.holes.len(), 4);
    assert_eq!(sketch.holes.iter().filter(|h| h.plated).count(), 2);
    assert_eq!(sketch.keepouts.len(), 1);
    assert!(!sketch.owns_outline(), "the board keeps its own outline");
    // Every baked item names the sketch entity it came from.
    let ids: Vec<Uuid> = sketch.sketch.entities.iter().map(|e| e.id.0).collect();
    for uuid in sketch.holes.iter().map(|h| h.uuid) {
        assert!(ids.contains(&uuid), "hole {uuid} has no circle");
    }
    assert!(ids.contains(&sketch.keepouts[0].uuid));
    // Slots, a bottom-side part and parts turned off the axes.
    let slots = board
        .footprints
        .iter()
        .flat_map(|f| &f.pads)
        .filter(|pad| pad.drill.as_ref().is_some_and(|d| d.shape == "oval"))
        .count();
    assert_eq!(slots, 3);
    assert!(board.footprints.iter().any(|f| f.layer == "BottomCopper"));
    assert!(board.footprints.iter().any(|f| f.rotation == 45.0));
    assert!(board.graphics.iter().any(|g| g.graphic_type == "arc"));
}

#[test]
fn real_board_projects_round_trip_and_carry_net_classes() {
    for real in generate() {
        let path = real_dir().join(format!("{}.snxprj", real.name));
        let bytes = fs::read(&path).expect("project file");
        let mut data = parse_project_bytes(&path, &bytes).expect("project parses");
        data.dir = String::new();
        assert_eq!(encode_project(&data).unwrap(), bytes, "{}", real.name);
        assert!(
            !data.net_classes.is_empty(),
            "{}: no net classes",
            real.name
        );
        assert_eq!(data.pcb_file, Some(format!("{}.snxpcb", real.name)));
    }
}

/// Even-odd point-in-polygon.
fn inside(poly: &[Point], q: Point) -> bool {
    let mut hit = false;
    for (i, a) in poly.iter().enumerate() {
        let b = poly[(i + 1) % poly.len()];
        if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            hit = !hit;
        }
    }
    hit
}

#[test]
fn every_item_lies_inside_the_board_outline() {
    for (name, ..) in COUNTS {
        let (_, board) = load(name);
        assert!(board.outline.len() >= 4, "{name}: no closed outline");
        let mut points: Vec<(String, Point)> = Vec::new();
        for fp in &board.footprints {
            points.push((format!("{} origin", fp.reference), fp.position));
            for pad in &fp.pads {
                let at = builder::pad_world(fp, pad);
                points.push((format!("{}.{}", fp.reference, pad.number), at));
            }
        }
        for s in &board.segments {
            points.push((format!("track {}", s.uuid), s.start));
            points.push((format!("track {}", s.uuid), s.end));
        }
        for v in &board.vias {
            points.push((format!("via {}", v.uuid), v.position));
        }
        for z in &board.zones {
            points.extend(z.outline.iter().map(|&q| (format!("zone {}", z.uuid), q)));
        }
        for t in &board.texts {
            points.push((format!("text {}", t.text), t.position));
        }
        let outside: Vec<&String> = points
            .iter()
            .filter(|(_, q)| !inside(&board.outline, *q))
            .map(|(what, _)| what)
            .collect();
        assert!(
            outside.is_empty(),
            "{name}: outside the outline: {outside:?}"
        );
    }
}

/// The board-space box of a footprint's courtyard: every courtyard point
/// placed, a circle as its centre plus radius.
fn courtyard_box(fp: &signex_types::pcb::Footprint) -> (f64, f64, f64, f64) {
    let mut corners = Vec::new();
    for g in fp
        .graphics
        .iter()
        .filter(|g| g.layer.ends_with("Courtyard"))
    {
        let r = g.radius;
        let local: Vec<Point> = match g.center {
            Some(c) => vec![
                Point::new(c.x - r, c.y - r),
                Point::new(c.x + r, c.y - r),
                Point::new(c.x + r, c.y + r),
                Point::new(c.x - r, c.y + r),
            ],
            None => g.points.clone(),
        };
        corners.extend(local.into_iter().map(|q| builder::to_world(fp, q)));
    }
    assert!(!corners.is_empty(), "{} has no courtyard", fp.reference);
    corners.iter().fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(x0, y0, x1, y1), q| (x0.min(q.x), y0.min(q.y), x1.max(q.x), y1.max(q.y)),
    )
}

#[test]
fn courtyards_on_one_side_do_not_overlap() {
    for (name, ..) in COUNTS {
        let (_, board) = load(name);
        let placed: Vec<_> = board
            .footprints
            .iter()
            .map(|fp| (fp.reference.as_str(), fp.layer.as_str(), courtyard_box(fp)))
            .collect();
        let mut clashes = Vec::new();
        for (i, (ra, la, a)) in placed.iter().enumerate() {
            for (rb, lb, b) in &placed[i + 1..] {
                let apart = a.2 <= b.0 || b.2 <= a.0 || a.3 <= b.1 || b.3 <= a.1;
                if la == lb && !apart {
                    clashes.push(format!("{ra}/{rb}"));
                }
            }
        }
        assert!(
            clashes.is_empty(),
            "{name}: courtyards overlap: {clashes:?}"
        );
    }
}

#[test]
fn the_linear_regulator_links_every_footprint_to_the_sample_schematic() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-app/assets/sample-project/linear-regulator.snxsch");
    let sheet = SnxSchematic::parse(&fs::read_to_string(path).unwrap())
        .unwrap()
        .sheet;
    let (_, board) = load("linear-regulator");
    assert_eq!(
        board.footprints.len(),
        linear_regulator::SCHEMATIC_UNITS.len()
    );
    for fp in &board.footprints {
        let link = fp.schematic_link.as_ref().expect("linked");
        assert!(link.sheet.chain().is_empty(), "root sheet");
        let [unit] = link.units.as_slice() else {
            panic!("{}: one unit", fp.reference)
        };
        let symbol = sheet
            .symbols
            .iter()
            .find(|s| s.uuid == *unit)
            .unwrap_or_else(|| panic!("{}: {unit} not on the sheet", fp.reference));
        assert_eq!(symbol.reference, fp.reference);
        assert_eq!(symbol.footprint, fp.footprint_id, "{}", fp.reference);
        assert_eq!(symbol.value, fp.value, "{}", fp.reference);
        assert_ne!(*unit, Uuid::nil());
    }
}
