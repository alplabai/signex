//! #2656: directives on the SVG page, and so on PDF pages and in print.

use signex_types::schematic::{BlanketPour, NetRules, directive_flag};
use uuid::Uuid;

use super::*;
use crate::Exporter;
use crate::pdf::PdfExporter;

fn xform() -> PageTransform {
    PageTransform {
        mm_to_unit: 1.0,
        translate_x: 0.0,
        translate_y: 0.0,
    }
}

fn blanket(class: Option<&str>) -> SchDirective {
    SchDirective::Blanket {
        uuid: Uuid::from_u128(1),
        start: Point::new(20.0, 20.0),
        end: Point::new(60.0, 40.0),
        net_class: class.map(str::to_string),
        erc_exclusions: Vec::new(),
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    }
}

fn mask() -> SchDirective {
    SchDirective::CompileMask {
        uuid: Uuid::from_u128(2),
        start: Point::new(20.0, 20.0),
        end: Point::new(60.0, 40.0),
    }
}

fn net_parameters() -> SchDirective {
    SchDirective::NetParameters {
        uuid: Uuid::from_u128(3),
        position: Point::new(100.0, 100.0),
        net_class: Some("POWER_IN".into()),
        diff_pair_class: None,
        rules: NetRules {
            width_mm: Some(0.8),
            clearance_mm: Some(0.3),
            ..Default::default()
        },
        show_on_flag: true,
    }
}

fn sheet(directives: Vec<SchDirective>) -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".into(),
        directives,
        ..Default::default()
    }
}

fn drawn(s: &SchematicSheet, opts: &PdfOptions) -> Vec<SvgElement> {
    let mut out = Vec::new();
    push_directives(&mut out, s, &xform(), &SchematicPalette::classic(), opts);
    out
}

/// The (text, font) of every text element.
fn texts(out: &[SvgElement]) -> Vec<(&str, &str)> {
    out.iter()
        .filter_map(|e| match e {
            SvgElement::Text {
                text, font_alias, ..
            } => Some((text.as_str(), *font_alias)),
            _ => None,
        })
        .collect()
}

/// How many separate strokes the path elements hold.
fn strokes(out: &[SvgElement]) -> usize {
    out.iter()
        .map(|e| match e {
            SvgElement::Path { commands, .. } => commands
                .iter()
                .filter(|c| matches!(c, SvgPathCommand::MoveTo(_)))
                .count(),
            _ => 0,
        })
        .sum()
}

#[test]
fn a_classed_blanket_prints_its_outline_and_its_flag_in_the_directive_colour() {
    let out = drawn(
        &sheet(vec![blanket(Some("SW_NODE"))]),
        &PdfOptions::default(),
    );
    let palette = SchematicPalette::classic();
    assert_eq!(texts(&out), [("Net Class = SW_NODE", "F4")]);
    for e in &out {
        match e {
            SvgElement::Path { style, .. } => {
                assert_eq!(style.stroke_rgb, Some(palette.no_connect));
                assert_eq!(style.fill_rgb, None);
            }
            SvgElement::Text { fill_rgb, .. } => assert_eq!(*fill_rgb, palette.no_connect),
        }
    }
    // A dashed outline: more marks than its four edges, plus the flag box.
    assert!(strokes(&out) > 8, "{}", strokes(&out));
}

#[test]
fn a_net_parameters_flag_prints_its_lines_where_the_canvas_draws_them() {
    let directive = net_parameters();
    let out = drawn(&sheet(vec![directive.clone()]), &PdfOptions::default());
    assert_eq!(
        texts(&out),
        [
            ("Net Class = POWER_IN", "F4"),
            ("Width 0.8 mm · Clr 0.3 mm", "F3")
        ]
    );
    let flag = directive_flag(&directive).expect("a flag");
    let origin = directive_flag_line_origin(&flag, 1);
    let second = out
        .iter()
        .filter_map(|e| match e {
            SvgElement::Text { x, y, .. } => Some((*x, *y)),
            _ => None,
        })
        .nth(1)
        .expect("two lines");
    assert_eq!(
        second,
        (xform().x(origin.x), xform().px_y(origin.y)),
        "the canvas's line origin"
    );
    // The stem and the flag box, solid.
    assert_eq!(strokes(&out), 5);
}

#[test]
fn the_blankets_and_parameter_sets_options_hide_their_directives() {
    let s = sheet(vec![blanket(Some("SW_NODE")), net_parameters()]);
    let no_blankets = PdfOptions {
        include_blankets: false,
        ..PdfOptions::default()
    };
    assert_eq!(
        texts(&drawn(&s, &no_blankets)),
        [
            ("Net Class = POWER_IN", "F4"),
            ("Width 0.8 mm · Clr 0.3 mm", "F3")
        ]
    );
    let no_parameters = PdfOptions {
        include_parameter_sets: false,
        ..PdfOptions::default()
    };
    assert_eq!(
        texts(&drawn(&s, &no_parameters)),
        [("Net Class = SW_NODE", "F4")]
    );
}

fn diff_pair() -> SchDirective {
    SchDirective::DiffPair {
        uuid: Uuid::from_u128(4),
        position: Point::new(80.0, 80.0),
        class: Some("USB_90R".into()),
    }
}

fn masks_and_pairs_on() -> PdfOptions {
    PdfOptions {
        include_compile_masks: true,
        include_diff_pairs: true,
        ..PdfOptions::default()
    }
}

/// Compile Masks and Differential Pairs print only behind their own options,
/// off by default (decision of 2026-11-25): with the default options neither,
/// flag included, is printed.
#[test]
fn a_compile_mask_and_a_diff_pair_are_not_printed_by_default() {
    let defaults = PdfOptions::default();
    assert!(!defaults.include_compile_masks && !defaults.include_diff_pairs);
    assert!(drawn(&sheet(vec![mask(), diff_pair()]), &defaults).is_empty());
    let with_blanket = drawn(&sheet(vec![mask(), blanket(None), diff_pair()]), &defaults);
    let blanket_only = drawn(&sheet(vec![blanket(None)]), &defaults);
    assert_eq!(format!("{with_blanket:?}"), format!("{blanket_only:?}"));
}

#[test]
fn a_compile_mask_prints_dotted_with_no_flag_when_its_option_is_on() {
    let dotted = drawn(&sheet(vec![mask()]), &masks_and_pairs_on());
    let dashed = drawn(&sheet(vec![blanket(None)]), &masks_and_pairs_on());
    assert!(texts(&dotted).is_empty() && texts(&dashed).is_empty());
    assert!(
        strokes(&dotted) > strokes(&dashed),
        "dots are shorter marks"
    );
}

#[test]
fn a_diff_pair_prints_its_two_strokes_and_its_flag_when_its_option_is_on() {
    let out = drawn(&sheet(vec![diff_pair()]), &masks_and_pairs_on());
    assert_eq!(texts(&out), [("Diff Pair · USB_90R", "F4")]);
    // The two strokes, the stem and the flag box.
    assert_eq!(strokes(&out), 7);
}

/// Each of the two options shows only its own kind.
#[test]
fn the_compile_masks_and_diff_pairs_options_are_apart() {
    let s = sheet(vec![mask(), diff_pair()]);
    let masks_only = PdfOptions {
        include_compile_masks: true,
        ..PdfOptions::default()
    };
    let pairs_only = PdfOptions {
        include_diff_pairs: true,
        ..PdfOptions::default()
    };
    assert_eq!(
        format!("{:?}", drawn(&s, &masks_only)),
        format!("{:?}", drawn(&sheet(vec![mask()]), &masks_and_pairs_on()))
    );
    assert_eq!(
        format!("{:?}", drawn(&s, &pairs_only)),
        format!(
            "{:?}",
            drawn(&sheet(vec![diff_pair()]), &masks_and_pairs_on())
        )
    );
}

/// A blanket without a class prints its dashed outline and no flag.
#[test]
fn a_classless_blanket_prints_a_dashed_outline_only() {
    let out = drawn(&sheet(vec![blanket(None)]), &PdfOptions::default());
    assert!(texts(&out).is_empty());
    assert!(strokes(&out) > 4, "dashes, not four edges");
}

/// With the default options the PDF page is the bare page byte for byte with
/// a mask and a pair on it; with their options on, it changes.
#[test]
fn a_pdf_page_is_unchanged_by_a_compile_mask_or_a_diff_pair() {
    let export = |directives: Vec<SchDirective>, opts: &PdfOptions| {
        let mut ctx = crate::pdf::tests::sample_ctx(1);
        ctx.sheets[0].schematic.directives = directives;
        PdfExporter.export(&ctx, opts).expect("export").bytes
    };
    let defaults = PdfOptions::default();
    assert!(export(vec![mask(), diff_pair()], &defaults) == export(Vec::new(), &defaults));
    let on = masks_and_pairs_on();
    assert!(export(vec![mask()], &on) != export(Vec::new(), &on));
    assert!(export(vec![diff_pair()], &on) != export(Vec::new(), &on));
}

#[test]
fn a_sheet_without_directives_prints_nothing_more() {
    assert!(drawn(&SchematicSheet::default(), &PdfOptions::default()).is_empty());
}

/// The PDF page draws them through the same elements: on, the page changes;
/// with its option off, it is the bare page byte for byte.
#[test]
fn a_pdf_page_draws_a_blanket_only_with_its_option_on() {
    let export = |directives: Vec<SchDirective>, include_blankets: bool| {
        let mut ctx = crate::pdf::tests::sample_ctx(1);
        ctx.sheets[0].schematic.directives = directives;
        let opts = PdfOptions {
            include_blankets,
            ..PdfOptions::default()
        };
        PdfExporter.export(&ctx, &opts).expect("export").bytes
    };
    let bare = export(Vec::new(), true);
    assert!(export(vec![blanket(Some("SW_NODE"))], true) != bare);
    assert!(export(vec![blanket(Some("SW_NODE"))], false) == bare);
}
