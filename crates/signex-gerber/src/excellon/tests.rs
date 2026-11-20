//! Whole-file reads of small drill texts.

use pcb_geom::{PointNm, Shape};

use super::header::split_tool;
use super::read_excellon;
use crate::file_function::{DrillKind, FileFunction};
use crate::image::{Image, ObjectKind, Units};
use crate::problem::ProblemKind;

fn kinds(image: &Image) -> Vec<ProblemKind> {
    image.problems.iter().map(|p| p.kind.clone()).collect()
}

fn hole(image: &Image, index: usize) -> (PointNm, Option<i64>, Option<PointNm>) {
    match image.objects[index].kind {
        ObjectKind::Hole {
            at,
            diameter,
            slot_to,
            ..
        } => (at, diameter, slot_to),
        ref other => panic!("not a hole: {other:?}"),
    }
}

#[test]
fn tool_words_split_into_number_and_size() {
    assert_eq!(split_tool("T1C0.300"), Some((1, Some("0.300"))));
    assert_eq!(split_tool("T02C"), Some((2, Some(""))));
    assert_eq!(split_tool("T03F200S65C1.0"), Some((3, Some("1.0"))));
    assert_eq!(split_tool("T4"), Some((4, None)));
    assert_eq!(split_tool("TCST,ON"), None);
    assert_eq!(split_tool("T1Q5"), None);
}

#[test]
fn a_metric_file_with_x2_reads_clean() {
    let text = "M48\n; #@! TF.FileFunction,NonPlated,1,4,NPTH\nFMAT,2\nMETRIC\nT1C1.000\n%\nG90\nG05\n\
                T1\nX1.250Y-2.000G85X2.750Y-2.000\nX5.0Y5.0\nT0\nM30\n";
    let image = read_excellon("n.drl", text);
    assert!(image.problems.is_empty(), "{:?}", image.problems);
    assert_eq!(image.units, Some(Units::Millimetres));
    assert_eq!(
        image.function,
        Some(FileFunction::Drill {
            plated: false,
            from: 1,
            to: 4,
            kind: DrillKind::Npth
        })
    );
    let (at, diameter, slot_to) = hole(&image, 0);
    assert_eq!(at, PointNm::new(1_250_000, -2_000_000));
    assert_eq!(diameter, Some(1_000_000));
    assert_eq!(slot_to, Some(PointNm::new(2_750_000, -2_000_000)));
    assert_eq!(
        image.objects[0].shapes,
        vec![Shape::Capsule {
            a: PointNm::new(1_250_000, -2_000_000),
            b: PointNm::new(2_750_000, -2_000_000),
            radius: 500_000
        }]
    );
    assert_eq!(hole(&image, 1).0, PointNm::new(5_000_000, 5_000_000));
    assert_eq!(image.apertures[&1].definition, "T1C1.000");
    assert_eq!(image.objects[1].at.line, 11);
}

#[test]
fn implied_inch_coordinates_follow_the_header() {
    let text = "M48\nINCH,TZ\nT1C0.035\n%\nT1\nX010000Y-5000\nY2500\nM30\n";
    let image = read_excellon("i.drl", text);
    // Only the lost file function.
    assert_eq!(kinds(&image), vec![ProblemKind::NoFileFunction]);
    assert_eq!(hole(&image, 0).0, PointNm::new(25_400_000, -12_700_000));
    // Y alone keeps X.
    assert_eq!(hole(&image, 1).0, PointNm::new(25_400_000, 6_350_000));
    assert_eq!(hole(&image, 0).1, Some(889_000));
    let lz = read_excellon("l.drl", "M48\nINCH,LZ\nT1C0.035\n%\nT1\nX01Y-005\nM30\n");
    // Leading zeros kept: "01" is 01.0000 inch and "-005" is -00.5000.
    assert_eq!(hole(&lz, 0).0, PointNm::new(25_400_000, -12_700_000));
}

#[test]
fn an_unknown_zero_mode_is_reported_once() {
    let text = "M48\nMETRIC\nT1C0.5\n%\nT1\nX1000Y1000\nX2000Y1000\nM30\n";
    let image = read_excellon("m.drl", text);
    let kinds = kinds(&image);
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == ProblemKind::MissingFormat)
            .count(),
        1
    );
    // 3.3 metric with leading zeros left out: 1000 is 1 mm.
    assert_eq!(hole(&image, 0).0, PointNm::new(1_000_000, 1_000_000));
}

#[test]
fn tools_without_a_size_or_definition_keep_their_holes() {
    let text = "M48\nMETRIC\nT1C0.8\nT2C\n%\nT2\nX1.0Y1.0\nX2.0Y1.0\nT7\nX3.0Y3.0\nM30\n";
    let image = read_excellon("t.drl", text);
    assert_eq!(image.objects.len(), 3);
    assert_eq!(hole(&image, 0).1, None);
    assert!(image.objects[0].shapes.is_empty());
    assert!(image.objects[2].shapes.is_empty());
    assert_eq!(
        kinds(&image),
        vec![
            ProblemKind::NoFileFunction,
            ProblemKind::ToolWithoutSize { tool: 2, holes: 2 },
            ProblemKind::UndefinedTool { tool: 7, holes: 1 },
        ]
    );
    assert_eq!(image.problems[1].at.line, 4);
    assert_eq!(image.problems[1].text, "T2C");
    assert_eq!(image.problems[2].at.line, 9);
}

#[test]
fn bad_header_lines_routing_and_a_missing_end_are_reported() {
    let text = "M48\nMETRIC\nWHAT,IS,THIS\nT1C0.5\n%\nT1\nG00X1.0Y1.0\nX1.0Y1.0\n";
    let image = read_excellon("b.drl", text);
    let kinds = kinds(&image);
    assert!(kinds.contains(&ProblemKind::BadExcellonHeader {
        text: "WHAT,IS,THIS".into()
    }));
    assert!(kinds.contains(&ProblemKind::UnsupportedCommand {
        command: "G00X1.0Y1.0".into()
    }));
    assert!(kinds.contains(&ProblemKind::MissingEndOfFile));
    assert_eq!(image.objects.len(), 1);
}
