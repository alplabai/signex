//! A board with every kind of copper the exporter writes, generated with
//! `signex_output::fab::generate` and read back: every image, drill file
//! and the job file read with no problem, the copper carries the board's
//! nets and pins, every hole has the board's size, and the job lists every
//! image.

use std::collections::{BTreeMap, BTreeSet};

use pcb_geom::PointNm;
use signex_gerber::{FileFunction, Image, JobFile, ObjectKind, Polarity, ReadFile, read_file};
use signex_output::fab::{FabExtras, FabFileKind, FabOptions, FabSet, generate};
use signex_types::pcb::{
    DrillDef, Footprint, LayerDef, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point,
    Segment, Via, ViaType, Zone,
};
use signex_types::pcb_arc::TrackArc;
use signex_types::pcb_free::FreePrimitives;
use signex_types::zone_pour::POUR_FILL;
use uuid::Uuid;

/// A net name with a comma and a character outside ASCII: both escaped.
const SIG: &str = "SIG,Ω";
const SMD_TOP: [&str; 3] = ["TopCopper", "TopSolderMask", "TopPaste"];
const THRU: [&str; 4] = [
    "TopCopper",
    "BottomCopper",
    "TopSolderMask",
    "BottomSolderMask",
];

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn nm(x: f64, y: f64) -> PointNm {
    PointNm::new((x * 1e6).round() as i64, (y * 1e6).round() as i64)
}

fn layer(id: u8, name: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.into(),
        layer_type: "signal".into(),
        display_name: String::new(),
    }
}

struct PadSpec<'a> {
    id: u128,
    number: &'a str,
    pad_type: PadType,
    shape: PadShape,
    at: Point,
    size: Point,
    layers: &'a [&'a str],
    net: Option<(u32, &'a str)>,
}

fn pad(spec: PadSpec) -> Pad {
    Pad {
        uuid: Uuid::from_u128(spec.id),
        number: spec.number.into(),
        pad_type: spec.pad_type,
        shape: spec.shape,
        position: spec.at,
        size: spec.size,
        drill: None,
        layers: spec.layers.iter().map(|l| l.to_string()).collect(),
        net: spec.net.map(|(number, name)| PadNet {
            number,
            name: name.into(),
        }),
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        net_locked: false,
        layer_sizes: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn drilled(pad: Pad, diameter: f64, shape: &str) -> Pad {
    Pad {
        drill: Some(DrillDef {
            diameter,
            shape: shape.into(),
        }),
        ..pad
    }
}

fn footprint(id: u128, reference: &str, at: Point, pads: Vec<Pad>) -> Footprint {
    Footprint {
        uuid: Uuid::from_u128(id),
        reference: reference.into(),
        value: "X".into(),
        footprint_id: "FP".into(),
        position: at,
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// U1's surface pads: a rectangle, a round rectangle (macro 21 + 1), a
/// turned oval (macro 21 + 1) and a polygon (macro 4).
fn surface_part() -> Footprint {
    let smd = |id, number, shape, x, size: Point, net| {
        pad(PadSpec {
            id,
            number,
            pad_type: PadType::Smd,
            shape,
            at: p(x, 0.0),
            size,
            layers: &SMD_TOP,
            net: Some(net),
        })
    };
    let rect = smd(101, "1", PadShape::Rect, -3.0, p(1.0, 1.5), (1, "GND"));
    let round = Pad {
        roundrect_ratio: 0.25,
        ..smd(102, "2", PadShape::RoundRect, -1.0, p(1.2, 0.8), (2, "VCC"))
    };
    let oval = Pad {
        rotation: 45.0,
        ..smd(103, "3", PadShape::Oval, 1.0, p(1.6, 0.8), (3, SIG))
    };
    let polygon = Pad {
        custom_outline: vec![p(-0.5, -0.5), p(0.5, -0.5), p(0.3, 0.5)],
        ..smd(104, "4", PadShape::Custom, 3.0, p(1.0, 1.0), (3, SIG))
    };
    footprint(10, "U1", p(10.0, 10.0), vec![rect, round, oval, polygon])
}

/// A round through-hole pin, an oval pin drilled as a slot, and a
/// non-plated mounting hole with no copper.
fn holed_parts() -> Vec<Footprint> {
    let thru = |id, shape, size: Point, net| {
        pad(PadSpec {
            id,
            number: "1",
            pad_type: PadType::Thru,
            shape,
            at: p(0.0, 0.0),
            size,
            layers: &THRU,
            net: Some(net),
        })
    };
    let pin = drilled(
        thru(201, PadShape::Circle, p(1.7, 1.7), (1, "GND")),
        1.0,
        "",
    );
    let slot = drilled(
        thru(301, PadShape::Oval, p(1.5, 3.0), (2, "VCC")),
        0.8,
        "oval",
    );
    let mounting = pad(PadSpec {
        id: 401,
        number: "1",
        pad_type: PadType::NpThru,
        shape: PadShape::Circle,
        at: p(0.0, 0.0),
        size: p(3.2, 3.2),
        layers: &[],
        net: None,
    });
    vec![
        footprint(20, "J1", p(20.0, 20.0), vec![pin]),
        footprint(30, "J2", p(30.0, 20.0), vec![slot]),
        footprint(40, "H1", p(35.0, 25.0), vec![drilled(mounting, 3.2, "")]),
    ]
}

/// A GND pour on the bottom over the whole board: the VCC via and pin cut
/// clear holes in it.
fn ground_pour() -> Zone {
    Zone {
        uuid: Uuid::from_u128(60),
        net: 1,
        net_name: "GND".into(),
        layer: "BottomCopper".into(),
        outline: vec![p(1.0, 1.0), p(39.0, 1.0), p(39.0, 29.0), p(1.0, 29.0)],
        holes: Vec::new(),
        priority: 0,
        fill_type: POUR_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.3,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        unknown: Default::default(),
    }
}

fn board() -> PcbBoard {
    let mut footprints = vec![surface_part()];
    footprints.extend(holed_parts());
    PcbBoard {
        stackup: None,
        uuid: Uuid::from_u128(1),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: vec![p(0.0, 0.0), p(40.0, 0.0), p(40.0, 30.0), p(0.0, 30.0)],
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: [(1, "GND"), (2, "VCC"), (3, SIG)]
            .map(|(number, name)| NetDef {
                number,
                name: name.into(),
            })
            .to_vec(),
        footprints,
        segments: vec![Segment {
            uuid: Uuid::from_u128(50),
            start: p(5.0, 5.0),
            end: p(15.0, 5.0),
            width: 0.25,
            layer: "TopCopper".into(),
            net: 2,
            unknown_columns: Default::default(),
            attributes: Default::default(),
            locked: false,
        }],
        vias: vec![Via {
            uuid: Uuid::from_u128(51),
            position: p(15.0, 5.0),
            diameter: 0.6,
            drill: 0.3,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: 2,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
            attributes: Default::default(),
            layer_sizes: None,
            locked: false,
        }],
        zones: vec![ground_pour()],
        graphics: Vec::new(),
        texts: Vec::new(),
        free: FreePrimitives {
            arcs: vec![TrackArc {
                uuid: Uuid::from_u128(52),
                start: p(5.0, 15.0),
                mid: p(7.5, 12.5),
                end: p(10.0, 15.0),
                width: 0.3,
                layer: "TopCopper".into(),
                net: 1,
            }],
            ..Default::default()
        },
        sketch: None,
        options: Default::default(),
        unknown: Default::default(),
    }
}

fn generated() -> FabSet {
    let board = board();
    generate(&board, &FabExtras::of_board(&board), &FabOptions::all("rt")).expect("generates")
}

/// Every Gerber and drill file as read, by name, with the function the
/// exporter gave it.
fn images(set: &FabSet) -> BTreeMap<String, (Image, String)> {
    set.files
        .iter()
        .filter_map(|file| {
            let function = match &file.kind {
                FabFileKind::Gerber { function, .. } | FabFileKind::Drill { function, .. } => {
                    function.clone()
                }
                _ => return None,
            };
            match read_file(&file.name, file.contents.as_bytes()) {
                ReadFile::Image(image) => Some((file.name.clone(), (image, function))),
                other => panic!("{} is not an image: {other:?}", file.name),
            }
        })
        .collect()
}

fn job(set: &FabSet) -> JobFile {
    let file = set
        .files
        .iter()
        .find(|f| f.kind == FabFileKind::Job)
        .expect("a job file");
    match read_file(&file.name, file.contents.as_bytes()) {
        ReadFile::Job(job) => job,
        other => panic!("the job file is not read: {other:?}"),
    }
}

#[test]
fn every_written_file_reads_back_with_no_problem() {
    let set = generated();
    let images = images(&set);
    assert!(images.len() >= 8, "{:?}", images.keys());
    for (name, (image, function)) in &images {
        assert!(image.problems.is_empty(), "{name}: {:?}", image.problems);
        assert_eq!(
            image.function,
            Some(FileFunction::parse(function)),
            "{name}"
        );
        assert!(
            image.objects.iter().all(|o| !o.shapes.is_empty()),
            "{name}: every object is built"
        );
    }
    assert!(set.notes.is_empty(), "{:?}", set.notes);
}

/// (designator, pin) to net of every pin flash on `image`.
fn pin_nets(image: &Image) -> BTreeMap<(String, String), String> {
    image
        .objects
        .iter()
        .filter_map(|o| {
            let pin = o.attributes.pin.clone()?;
            assert_eq!(o.attributes.component.as_ref(), Some(&pin.0));
            Some((pin, o.attributes.net.clone().unwrap_or_default()))
        })
        .collect()
}

fn pin(designator: &str, number: &str, net: &str) -> ((String, String), String) {
    (
        (designator.to_string(), number.to_string()),
        net.to_string(),
    )
}

#[test]
fn the_copper_carries_the_boards_nets_and_pins() {
    let images = images(&generated());
    let (top, _) = &images["rt-copper-l1-top.gbr"];
    assert_eq!(
        pin_nets(top),
        BTreeMap::from([
            pin("J1", "1", "GND"),
            pin("J2", "1", "VCC"),
            pin("U1", "1", "GND"),
            pin("U1", "2", "VCC"),
            pin("U1", "3", SIG),
            pin("U1", "4", SIG),
        ])
    );
    let nets: BTreeSet<&str> = top
        .objects
        .iter()
        .filter_map(|o| o.attributes.net.as_deref())
        .collect();
    assert_eq!(nets, BTreeSet::from(["GND", "VCC", SIG]));
    let (bottom, _) = &images["rt-copper-l2-bottom.gbr"];
    assert_eq!(
        pin_nets(bottom),
        BTreeMap::from([pin("J1", "1", "GND"), pin("J2", "1", "VCC")])
    );
    // The pour: GND regions, with clear holes where the VCC copper is.
    let regions: Vec<_> = bottom
        .objects
        .iter()
        .filter(|o| matches!(o.kind, ObjectKind::Region { .. }))
        .collect();
    assert!(regions.iter().any(|o| o.polarity == Polarity::Dark));
    assert!(regions.iter().any(|o| o.polarity == Polarity::Clear));
    assert!(
        regions
            .iter()
            .all(|o| o.attributes.net.as_deref() == Some("GND"))
    );
}

/// (position, diameter, slot end) of every hole of a drill image.
fn holes(image: &Image) -> BTreeSet<(PointNm, Option<i64>, Option<PointNm>)> {
    image
        .objects
        .iter()
        .map(|o| match o.kind {
            ObjectKind::Hole {
                at,
                diameter,
                slot_to,
                ..
            } => (at, diameter, slot_to),
            ref other => panic!("not a hole: {other:?}"),
        })
        .collect()
}

#[test]
fn every_hole_has_the_boards_size() {
    let images = images(&generated());
    let (pth, _) = &images["rt-pth.drl"];
    assert_eq!(
        holes(pth),
        BTreeSet::from([
            (nm(15.0, -5.0), Some(300_000), None),
            (nm(20.0, -20.0), Some(1_000_000), None),
            (nm(30.0, -19.25), Some(800_000), Some(nm(30.0, -20.75))),
        ])
    );
    let (npth, _) = &images["rt-npth.drl"];
    assert_eq!(
        holes(npth),
        BTreeSet::from([(nm(35.0, -25.0), Some(3_200_000), None)])
    );
}

#[test]
fn the_job_lists_every_image() {
    let set = generated();
    let images = images(&set);
    let job = job(&set);
    assert_eq!(job.project_name.as_deref(), Some("rt"));
    assert_eq!(job.copper_layers, Some(2));
    assert_eq!(job.board_size_nm, Some((40_000_000, 30_000_000)));
    let listed: BTreeMap<&str, &FileFunction> = job
        .entries
        .iter()
        .map(|e| (e.path.as_str(), &e.function))
        .collect();
    assert_eq!(listed.len(), images.len());
    for (name, (image, _)) in &images {
        assert_eq!(
            listed.get(name.as_str()).copied(),
            image.function.as_ref(),
            "{name}"
        );
    }
    assert!(job.other_files.iter().any(|f| f == "rt-drill-table.csv"));
}
