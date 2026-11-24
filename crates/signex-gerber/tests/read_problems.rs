//! Files that cannot be read in full: each problem names its file and line,
//! and nothing is dropped. The fixtures in `tests/fixtures/problems/` are
//! written by hand to read like a package back from a board house.

use pcb_geom::{PointNm, Shape};
use signex_gerber::{
    CopperSide, FileFunction, FileKind, Image, ObjectKind, Polarity, ProblemKind, ReadFile,
    Severity, read_file, sniff_kind,
};

fn bytes(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/problems/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn image(name: &str) -> Image {
    match read_file(name, &bytes(name)) {
        ReadFile::Image(image) => image,
        other => panic!("{name} is not an image: {other:?}"),
    }
}

fn kinds(image: &Image) -> Vec<ProblemKind> {
    image.problems.iter().map(|p| p.kind.clone()).collect()
}

fn inch(tenth_thousandths: i64) -> i64 {
    tenth_thousandths * 2_540
}

#[test]
fn an_unknown_macro_is_one_error_at_its_definition_and_its_flashes_stay() {
    let image = image("copper-l2-thermal.gbr");
    assert_eq!(
        kinds(&image),
        vec![ProblemKind::UnknownApertureMacro {
            name: "THERMAL80".into(),
            flashes: 2
        }]
    );
    let problem = &image.problems[0];
    assert_eq!(problem.at.file, "copper-l2-thermal.gbr");
    assert_eq!(problem.at.line, 25);
    assert_eq!(problem.text, "%ADD21THERMAL80,2.3X1.5X0.3*%");
    assert_eq!(problem.severity(), Severity::Error);
    assert_eq!(
        image.function,
        Some(FileFunction::Copper {
            number: 2,
            side: CopperSide::Inner
        })
    );
    // The plane, the two thermal flashes and the via: nothing dropped.
    assert_eq!(image.objects.len(), 4);
    let unbuilt: Vec<(PointNm, u32)> = image
        .objects
        .iter()
        .filter(|o| o.shapes.is_empty())
        .map(|o| match o.kind {
            ObjectKind::Flash { at } => (at, o.at.line),
            ref other => panic!("not a flash: {other:?}"),
        })
        .collect();
    assert_eq!(
        unbuilt,
        vec![
            (PointNm::new(6_200_000, 11_500_000), 28),
            (PointNm::new(26_400_000, 6_000_000), 29)
        ]
    );
    assert_eq!(
        image.apertures[&21].function.as_deref(),
        Some("ThermalReliefPad")
    );
    // The rest of the file is drawn.
    assert!(matches!(image.objects[0].kind, ObjectKind::Region { .. }));
    assert_eq!(image.objects[0].attributes.net.as_deref(), Some("GND"));
    assert_eq!(image.objects[3].attributes.net.as_deref(), Some("VIN_12V"));
    assert_eq!(
        image.objects[3].shapes,
        vec![Shape::Disc {
            center: PointNm::new(12_000_000, 12_000_000),
            radius: 300_000
        }]
    );
}

#[test]
fn a_tool_without_a_size_keeps_its_holes_without_a_diameter() {
    let image = image("npth.drl");
    assert_eq!(image.kind, FileKind::Excellon);
    assert_eq!(
        kinds(&image),
        vec![ProblemKind::ToolWithoutSize { tool: 2, holes: 2 }]
    );
    assert_eq!(image.problems[0].at.line, 6);
    assert_eq!(image.problems[0].text, "T02C");
    assert_eq!(image.objects.len(), 6);
    let holes: Vec<(PointNm, Option<i64>, bool)> = image
        .objects
        .iter()
        .map(|o| match o.kind {
            ObjectKind::Hole { at, diameter, .. } => (at, diameter, o.shapes.is_empty()),
            ref other => panic!("not a hole: {other:?}"),
        })
        .collect();
    assert_eq!(
        holes[0],
        (PointNm::new(2_500_000, 2_500_000), Some(3_200_000), false)
    );
    assert_eq!(holes[4], (PointNm::new(6_200_000, 11_500_000), None, true));
    assert_eq!(holes[5], (PointNm::new(26_400_000, 6_000_000), None, true));
}

#[test]
fn a_file_without_an_x2_function_is_a_warning() {
    let image = image("silk-bottom.gbr");
    assert_eq!(kinds(&image), vec![ProblemKind::NoFileFunction]);
    assert_eq!(image.problems[0].severity(), Severity::Warning);
    assert!(!image.has_errors());
    assert_eq!(image.function, None);
    assert_eq!(image.objects.len(), 1);
}

#[test]
fn an_inch_file_with_arcs_and_a_clear_region_reads_to_exact_nm() {
    let image = image("inch-arcs.gbr");
    assert!(image.problems.is_empty(), "{:?}", image.problems);
    let p = |x: i64, y: i64| PointNm::new(inch(x), inch(y));
    let kinds: Vec<ObjectKind> = image.objects.iter().map(|o| o.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            ObjectKind::Arc {
                from: p(10_000, 0),
                to: p(0, 10_000),
                center: p(0, 0),
                ccw: true
            },
            ObjectKind::Arc {
                from: p(0, 10_000),
                to: p(10_000, 20_000),
                center: p(10_000, 10_000),
                ccw: false
            },
            // G74: the unsigned offsets give the centre making a quarter.
            ObjectKind::Arc {
                from: p(10_000, 20_000),
                to: p(20_000, 10_000),
                center: p(10_000, 10_000),
                ccw: false
            },
            ObjectKind::Region {
                outline: vec![
                    p(5_000, 5_000),
                    p(15_000, 5_000),
                    p(15_000, 15_000),
                    p(5_000, 15_000)
                ]
            },
            ObjectKind::Flash { at: p(30_000, 0) },
        ]
    );
    assert_eq!(image.objects[3].polarity, Polarity::Clear);
    assert_eq!(image.objects[4].polarity, Polarity::Dark);
    assert_eq!(
        image.objects[4].shapes,
        vec![Shape::Rect {
            center: p(30_000, 0),
            half_w: 762_000,
            half_h: 508_000,
            rotation_deg: 0.0
        }]
    );
    // The pen is 0.01 inch.
    assert!(matches!(
        image.objects[0].shapes[0],
        Shape::Capsule {
            radius: 127_000,
            ..
        }
    ));
}

#[test]
fn a_file_missing_its_end_is_a_warning_and_still_drawn() {
    let image = image("paste-no-end.gbr");
    assert_eq!(kinds(&image), vec![ProblemKind::MissingEndOfFile]);
    assert_eq!(image.problems[0].severity(), Severity::Warning);
    assert_eq!(image.problems[0].at.line, 6);
    assert_eq!(image.objects.len(), 1);
}

#[test]
fn bytes_that_are_not_text_are_not_read() {
    let ReadFile::NotRead { file, problem } = read_file("garbage.gbr", &bytes("garbage.gbr"))
    else {
        panic!("garbage is not read");
    };
    assert_eq!(file, "garbage.gbr");
    assert_eq!(problem.kind, ProblemKind::NotText);
    assert_eq!(problem.at.line, 1);
    assert_eq!(problem.severity(), Severity::Error);
}

#[test]
fn a_readme_is_listed_as_not_read() {
    let readme = bytes("readme.txt");
    assert_eq!(sniff_kind("readme.txt", &readme), FileKind::NotRead);
    let ReadFile::NotRead { problem, .. } = read_file("readme.txt", &readme) else {
        panic!("a readme is not read");
    };
    assert_eq!(problem.kind, ProblemKind::NotAFabricationFile);
    assert_eq!(problem.severity(), Severity::Info);
    assert_eq!(problem.text, "Fabrication package for dcdc-buck revB.");
}
