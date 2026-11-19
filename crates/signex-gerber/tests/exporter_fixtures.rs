//! The files Signex's own exporter writes, read back. Each fixture in
//! `tests/fixtures/signex-export/` is a byte copy of a `signex-output`
//! golden file (`src/fab/testdata/`), and the first test fails when the two
//! differ: a golden refresh must refresh the fixture, so the reader is
//! always checked against what the exporter writes today.

use std::collections::BTreeSet;

use pcb_geom::{PointNm, Shape};
use signex_gerber::{
    CopperSide, DrillKind, FileFunction, FileKind, FilePolarity, Image, ObjectKind, ReadFile, Side,
    Units, read_file, sniff_kind,
};

const FILES: [&str; 4] = [
    "fab-copper-l1-top.gbr",
    "fab-soldermask-top.gbr",
    "fab-profile.gbr",
    "fab-pth.drl",
];

fn read_bytes(dir: &str, name: &str) -> Vec<u8> {
    let path = format!("{}/{dir}/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn fixture(name: &str) -> Vec<u8> {
    read_bytes("tests/fixtures/signex-export", name)
}

fn image(name: &str) -> Image {
    match read_file(name, &fixture(name)) {
        ReadFile::Image(image) => image,
        other => panic!("{name} is not an image: {other:?}"),
    }
}

fn mm(value: f64) -> i64 {
    (value * 1e6).round() as i64
}

fn at(x: f64, y: f64) -> PointNm {
    PointNm::new(mm(x), mm(y))
}

#[test]
fn every_fixture_is_a_copy_of_its_exporter_golden() {
    for name in FILES {
        assert_eq!(
            fixture(name),
            read_bytes("../signex-output/src/fab/testdata", name),
            "{name}: copy the signex-output golden over the fixture"
        );
    }
}

#[test]
fn every_fixture_reads_with_no_problem() {
    for name in FILES {
        let image = image(name);
        assert!(image.problems.is_empty(), "{name}: {:?}", image.problems);
        assert_eq!(image.generation_software.as_deref(), Some("Alp Lab,Signex"));
        assert_eq!(image.units, Some(Units::Millimetres));
        assert!(
            image.objects.iter().all(|o| !o.shapes.is_empty()),
            "{name}: every object is built"
        );
    }
    assert_eq!(
        sniff_kind("fab-pth.drl", &fixture("fab-pth.drl")),
        FileKind::Excellon
    );
    assert_eq!(
        sniff_kind("fab-profile.gbr", &fixture("fab-profile.gbr")),
        FileKind::Gerber
    );
}

#[test]
fn the_top_copper_holds_the_pads_the_track_and_the_via() {
    let image = image("fab-copper-l1-top.gbr");
    assert_eq!(
        image.function,
        Some(FileFunction::Copper {
            number: 1,
            side: CopperSide::Top
        })
    );
    assert_eq!(image.polarity, FilePolarity::Positive);
    assert_eq!(image.objects.len(), 5);
    let flashes: Vec<_> = image
        .objects
        .iter()
        .filter(|o| matches!(o.kind, ObjectKind::Flash { .. }))
        .collect();
    assert_eq!(flashes.len(), 4);
    let pins: Vec<(String, String)> = flashes
        .iter()
        .filter_map(|o| o.attributes.pin.clone())
        .collect();
    let pin = |d: &str, n: &str| (d.to_string(), n.to_string());
    assert_eq!(pins, vec![pin("R1", "1"), pin("R1", "2"), pin("J1", "1")]);
    let nets: BTreeSet<&str> = image
        .objects
        .iter()
        .filter_map(|o| o.attributes.net.as_deref())
        .collect();
    assert_eq!(nets, BTreeSet::from(["GND", "VCC"]));
    assert_eq!(
        flashes[0].shapes,
        vec![Shape::Rect {
            center: at(4.0, -5.0),
            half_w: 500_000,
            half_h: 600_000,
            rotation_deg: 0.0
        }]
    );
    assert_eq!(flashes[0].attributes.component.as_deref(), Some("R1"));
    assert_eq!(flashes[2].attributes.net.as_deref(), Some("GND"));
    assert_eq!(
        flashes[2].shapes,
        vec![Shape::Disc {
            center: at(10.0, -8.0),
            radius: 800_000
        }]
    );
    // The track, then the via on its end.
    assert_eq!(
        image.objects[3].kind,
        ObjectKind::Line {
            from: at(6.0, -5.0),
            to: at(15.0, -5.0)
        }
    );
    assert_eq!(image.objects[3].attributes.net.as_deref(), Some("VCC"));
    assert_eq!(image.objects[3].attributes.pin, None);
    assert_eq!(
        image.objects[4].kind,
        ObjectKind::Flash { at: at(15.0, -5.0) }
    );
    assert_eq!(image.objects[4].aperture, Some(13));
    let functions: Vec<Option<&str>> = image
        .apertures
        .values()
        .map(|a| a.function.as_deref())
        .collect();
    assert_eq!(
        functions,
        vec![
            Some("SMDPad,CuDef"),
            Some("ComponentPad"),
            Some("Conductor"),
            Some("ViaPad")
        ]
    );
    assert_eq!(image.apertures[&10].definition, "%ADD10R,1X1.2*%");
    assert_eq!(image.apertures[&10].at.line, 7);
}

#[test]
fn the_top_mask_is_negative_with_three_openings() {
    let image = image("fab-soldermask-top.gbr");
    assert_eq!(image.function, Some(FileFunction::Soldermask(Side::Top)));
    assert_eq!(image.polarity, FilePolarity::Negative);
    let openings: Vec<PointNm> = image
        .objects
        .iter()
        .map(|o| match o.kind {
            ObjectKind::Flash { at } => at,
            ref other => panic!("not a flash: {other:?}"),
        })
        .collect();
    assert_eq!(openings, vec![at(4.0, -5.0), at(6.0, -5.0), at(10.0, -8.0)]);
    assert!(
        image
            .objects
            .iter()
            .all(|o| o.attributes == Default::default())
    );
}

#[test]
fn the_profile_is_a_closed_20_by_10_rectangle() {
    let image = image("fab-profile.gbr");
    assert_eq!(
        image.function,
        Some(FileFunction::Profile { plated: false })
    );
    let corners = [at(0.0, 0.0), at(20.0, 0.0), at(20.0, -10.0), at(0.0, -10.0)];
    let lines: Vec<ObjectKind> = image.objects.iter().map(|o| o.kind.clone()).collect();
    let expected: Vec<ObjectKind> = (0..4)
        .map(|i| ObjectKind::Line {
            from: corners[i],
            to: corners[(i + 1) % 4],
        })
        .collect();
    assert_eq!(lines, expected);
    // The 0.1 mm pen reaches 0.05 mm past the outline.
    assert_eq!(image.bounds(), Some((at(-0.05, -10.05), at(20.05, 0.05))));
}

#[test]
fn the_plated_drill_file_holds_both_tools() {
    let image = image("fab-pth.drl");
    assert_eq!(image.kind, FileKind::Excellon);
    assert_eq!(
        image.function,
        Some(FileFunction::Drill {
            plated: true,
            from: 1,
            to: 2,
            kind: DrillKind::Pth
        })
    );
    let holes: Vec<(u32, PointNm, Option<i64>)> = image
        .objects
        .iter()
        .map(|o| match o.kind {
            ObjectKind::Hole {
                at, diameter, tool, ..
            } => (tool, at, diameter),
            ref other => panic!("not a hole: {other:?}"),
        })
        .collect();
    assert_eq!(
        holes,
        vec![
            (1, at(15.0, -5.0), Some(300_000)),
            (2, at(10.0, -8.0), Some(800_000))
        ]
    );
    assert_eq!(image.apertures[&1].definition, "T1C0.300");
    assert_eq!(image.apertures[&2].definition, "T2C0.800");
}
