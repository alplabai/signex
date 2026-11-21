//! Whole-file reads of small Gerber texts. Fixture files and the exporter
//! round trip are in `tests/`.

use pcb_geom::{PointNm, Shape};

use super::read_gerber;
use crate::file_function::{CopperSide, FileFunction};
use crate::image::{FilePolarity, Image, ObjectKind, Polarity, Units};
use crate::problem::{ProblemKind, Severity};

const HEAD: &str = "%TF.FileFunction,Copper,L1,Top*%\n%FSLAX46Y46*%\n%MOMM*%\n";

fn read(body: &str) -> Image {
    read_gerber("t.gbr", &format!("{HEAD}{body}M02*\n"))
}

fn p(x: i64, y: i64) -> PointNm {
    PointNm::new(x, y)
}

fn kinds(image: &Image) -> Vec<ProblemKind> {
    image.problems.iter().map(|p| p.kind.clone()).collect()
}

#[test]
fn a_clean_file_has_no_problems() {
    let image = read("%ADD10C,0.5*%\nD10*\nX1000000Y2000000D03*\n");
    assert!(image.problems.is_empty(), "{:?}", image.problems);
    assert_eq!(
        image.function,
        Some(FileFunction::Copper {
            number: 1,
            side: CopperSide::Top
        })
    );
    assert_eq!(image.units, Some(Units::Millimetres));
    assert_eq!(image.objects.len(), 1);
    assert_eq!(
        image.objects[0].shapes,
        vec![Shape::Disc {
            center: p(1_000_000, 2_000_000),
            radius: 250_000
        }]
    );
    assert_eq!(image.objects[0].at.line, 6);
    assert_eq!(image.apertures[&10].definition, "%ADD10C,0.5*%");
}

#[test]
fn coordinates_are_modal_and_operations_repeat() {
    let image = read("%ADD10C,0.1*%\nD10*\nX0Y0D02*\nX1000D01*\nY1000*\n");
    let lines: Vec<_> = image.objects.iter().map(|o| o.kind.clone()).collect();
    assert_eq!(
        lines,
        vec![
            ObjectKind::Line {
                from: p(0, 0),
                to: p(1000, 0)
            },
            ObjectKind::Line {
                from: p(1000, 0),
                to: p(1000, 1000)
            },
        ]
    );
    assert!(image.problems.is_empty(), "{:?}", image.problems);
}

#[test]
fn an_arc_keeps_its_centre_and_is_drawn_as_a_capsule_chain() {
    let image =
        read("%ADD10C,0.2*%\nD10*\nG75*\nX1000000Y0D02*\nG03*\nX0Y1000000I-1000000J0D01*\n");
    let object = &image.objects[0];
    assert_eq!(
        object.kind,
        ObjectKind::Arc {
            from: p(1_000_000, 0),
            to: p(0, 1_000_000),
            center: p(0, 0),
            ccw: true
        }
    );
    assert!(object.shapes.len() > 4);
    assert!(object.shapes.iter().all(|s| matches!(
        s,
        Shape::Capsule {
            radius: 100_000,
            ..
        }
    )));
    let Some(Shape::Capsule { b, .. }) = object.shapes.last() else {
        panic!("a capsule");
    };
    assert_eq!(*b, p(0, 1_000_000));
}

#[test]
fn regions_close_and_clear_polarity_is_kept() {
    let image = read(
        "G36*\nX0Y0D02*\nG01*\nX4000000Y0D01*\nX4000000Y4000000D01*\nX0Y4000000D01*\nX0Y0D01*\nG37*\n\
         %LPC*%\nG36*\nX1000000Y1000000D02*\nX2000000Y1000000D01*\nX2000000Y2000000D01*\nG37*\n%LPD*%\n",
    );
    assert_eq!(image.objects.len(), 2);
    let ObjectKind::Region { outline } = &image.objects[0].kind else {
        panic!("a region");
    };
    assert_eq!(outline.len(), 4);
    assert_eq!(image.objects[0].polarity, Polarity::Dark);
    assert!(matches!(
        image.objects[0].shapes[0],
        Shape::Polygon { radius: 0, .. }
    ));
    assert_eq!(image.objects[1].polarity, Polarity::Clear);
    // The second contour does not return to its start.
    assert_eq!(kinds(&image), vec![ProblemKind::UnclosedRegion]);
    assert_eq!(image.problems[0].text, "G37*");
}

#[test]
fn macro_apertures_build_from_their_parameters() {
    let image = read(
        "%AMBOX*\n0 a rectangle of $1 by $2*\n21,1,$1,$2,0,0,0*%\n%ADD11BOX,2X1*%\nD11*\nX0Y0D03*\n",
    );
    assert!(image.problems.is_empty(), "{:?}", image.problems);
    assert_eq!(
        image.objects[0].shapes,
        vec![Shape::Rect {
            center: p(0, 0),
            half_w: 1_000_000,
            half_h: 500_000,
            rotation_deg: 0.0
        }]
    );
}

#[test]
fn an_unknown_macro_and_an_undefined_aperture_are_counted_not_dropped() {
    let image = read("%ADD21NOPE,1X2*%\nD21*\nX0Y0D03*\nX10Y0D03*\nD15*\nX20Y0D03*\n");
    assert_eq!(image.objects.len(), 3);
    assert!(image.objects.iter().all(|o| o.shapes.is_empty()));
    assert_eq!(
        kinds(&image),
        vec![
            ProblemKind::UnknownApertureMacro {
                name: "NOPE".into(),
                flashes: 2
            },
            ProblemKind::UndefinedAperture {
                d_code: 15,
                uses: 1
            },
        ]
    );
    assert_eq!(image.problems[0].at.line, 4);
    assert_eq!(image.problems[1].at.line, 8);
    assert!(image.has_errors());
}

#[test]
fn step_and_repeat_places_every_copy() {
    let image = read("%ADD10C,1*%\nD10*\n%SRX3Y2I5.0J4.0*%\nX0Y0D03*\n%SR*%\n");
    let at: Vec<_> = image
        .objects
        .iter()
        .map(|o| match o.kind {
            ObjectKind::Flash { at } => at,
            _ => panic!("a flash"),
        })
        .collect();
    assert_eq!(at.len(), 6);
    assert!(at.contains(&p(10_000_000, 4_000_000)));
    assert!(image.problems.is_empty(), "{:?}", image.problems);
}

#[test]
fn a_block_aperture_is_flashed_where_it_is_placed() {
    let image = read(
        "%ADD10C,1*%\n%ABD12*%\nD10*\nX0Y0D03*\nX1000000Y0D03*\n%AB*%\nD12*\nX5000000Y5000000D03*\n",
    );
    let at: Vec<_> = image
        .objects
        .iter()
        .map(|o| match o.kind {
            ObjectKind::Flash { at } => at,
            _ => panic!("a flash"),
        })
        .collect();
    assert_eq!(at, vec![p(5_000_000, 5_000_000), p(6_000_000, 5_000_000)]);
    assert!(image.problems.is_empty(), "{:?}", image.problems);
}

#[test]
fn object_attributes_attach_and_are_deleted() {
    let image = read(
        "%ADD10C,1*%\nD10*\n%TO.N,A\\u002CB*%\n%TO.P,U1,3*%\n%TO.C,U1*%\nX0Y0D03*\n%TD.N*%\nX1Y0D03*\n%TD*%\nX2Y0D03*\n",
    );
    let first = &image.objects[0].attributes;
    assert_eq!(first.net.as_deref(), Some("A,B"));
    assert_eq!(first.pin, Some(("U1".into(), "3".into())));
    assert_eq!(first.component.as_deref(), Some("U1"));
    assert_eq!(image.objects[1].attributes.net, None);
    assert_eq!(image.objects[1].attributes.component.as_deref(), Some("U1"));
    assert_eq!(image.objects[2].attributes, Default::default());
}

#[test]
fn unsupported_and_malformed_commands_are_reported_with_their_line() {
    let image = read("%ADD10C,1*%\nD10*\n%LR45*%\nQ7*\nX0Y0D03*\n%IPPOS*%\n%MIA0B0*%\n");
    assert_eq!(
        kinds(&image),
        vec![
            ProblemKind::UnsupportedCommand {
                command: "LR45".into()
            },
            ProblemKind::UnsupportedCommand {
                command: "Q7".into()
            },
        ]
    );
    assert_eq!(
        image.problems.iter().map(|p| p.at.line).collect::<Vec<_>>(),
        vec![6, 7]
    );
    // The flash after them is still drawn.
    assert_eq!(image.objects.len(), 1);
}

#[test]
fn a_file_without_format_units_function_or_end_says_so() {
    let image = read_gerber("bare.gbr", "%ADD10C,1*%\nD10*\nX1Y1D03*\n");
    let kinds = kinds(&image);
    assert!(kinds.contains(&ProblemKind::MissingUnits), "{kinds:?}");
    assert!(kinds.contains(&ProblemKind::MissingFormat), "{kinds:?}");
    assert!(kinds.contains(&ProblemKind::NoFileFunction), "{kinds:?}");
    assert!(kinds.contains(&ProblemKind::MissingEndOfFile), "{kinds:?}");
    let warnings = image
        .problems
        .iter()
        .filter(|p| p.severity() == Severity::Warning)
        .count();
    assert_eq!(warnings, 2);
    assert_eq!(image.objects.len(), 1);
}

#[test]
fn file_attributes_fill_the_image() {
    let image = read_gerber(
        "m.gbr",
        "%TF.GenerationSoftware,Vendor,Tool,1.0*%\n%TF.ProjectId,board,1234,A*%\n\
         %TF.FileFunction,Soldermask,Bot*%\n%TF.FilePolarity,Negative*%\n%FSLAX46Y46*%\n%MOMM*%\nM02*\n",
    );
    assert!(image.problems.is_empty(), "{:?}", image.problems);
    assert_eq!(
        image.generation_software.as_deref(),
        Some("Vendor,Tool,1.0")
    );
    assert_eq!(image.project_id.as_deref(), Some("board,1234,A"));
    assert_eq!(image.polarity, FilePolarity::Negative);
}
