//! Tests for PDF layers (optional content groups).
use std::collections::BTreeSet;

use signex_types::schematic::{
    HarnessConnector, HarnessEntry, HarnessWire, Junction, Label, LabelType, Point, Symbol, Wire,
};

use super::tests::sample_ctx;
use super::*;
use crate::ExportContext;

/// One sheet with a wire and its junction, a signal harness (wire and a
/// connector with one entry), a part, a power port and a net label.
pub(super) fn layered_ctx() -> ExportContext {
    let mut ctx = sample_ctx(1);
    let sheet = &mut ctx.sheets[0].schematic;
    sheet.wires.push(Wire {
        uuid: uuid::Uuid::from_u128(1),
        start: Point::new(20.0, 20.0),
        end: Point::new(60.0, 20.0),
        ..Default::default()
    });
    sheet.junctions.push(Junction {
        uuid: uuid::Uuid::from_u128(4),
        position: Point::new(40.0, 20.0),
        diameter: 0.0,
        minted: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    });
    sheet.harness.connectors.push(HarnessConnector {
        uuid: uuid::Uuid::from_u128(5),
        harness_type: "SPI_BUS".into(),
        position: Point::new(20.0, 80.0),
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: vec![HarnessEntry {
            uuid: uuid::Uuid::from_u128(6),
            name: "SCK".into(),
        }],
        locked: false,
    });
    sheet.harness.wires.push(HarnessWire {
        uuid: uuid::Uuid::from_u128(7),
        start: Point::new(40.0, 82.0),
        end: Point::new(60.0, 82.0),
        locked: false,
    });
    sheet.symbols.push(Symbol {
        reference: "R1".to_string(),
        position: Point::new(80.0, 60.0),
        ..Default::default()
    });
    sheet.symbols.push(Symbol {
        reference: "#PWR01".to_string(),
        value: "GND".to_string(),
        is_power: true,
        position: Point::new(120.0, 60.0),
        ..Default::default()
    });
    sheet
        .labels
        .push(Label::new("VCC", Point::new(30.0, 20.0), LabelType::Net));
    ctx
}

/// The content-stream operators of page `page_index` (sheet 0).
pub(super) fn page_ops(ctx: &ExportContext, opts: &PdfOptions, page_index: usize) -> String {
    let tables = build_expression_tables(&ctx.sheets, ctx.netlist.as_ref());
    let (_, page_h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    let mut fonts = FontUsage::new();
    let content = build_page_content(
        &ctx.sheets[0],
        page_index,
        opts,
        ctx,
        (page_h_mm * MM_TO_PT) as f32,
        &tables,
        &mut fonts,
    )
    .expect("page content");
    String::from_utf8(content.bytes).expect("operators are ASCII")
}

fn layered(hidden: &[PdfLayer], watermark: bool) -> PdfOptions {
    PdfOptions {
        layers: Some(PdfLayerVisibility {
            hidden_on_open: hidden.iter().copied().collect::<BTreeSet<_>>(),
        }),
        watermark: watermark.then(Watermark::default),
        ..PdfOptions::default()
    }
}

fn export_text(ctx: &ExportContext, opts: &PdfOptions) -> String {
    let out = PdfExporter.export(ctx, opts).expect("export");
    String::from_utf8_lossy(&out.bytes).into_owned()
}

/// Every `/OCG` object as (`"N 0 R"`, its `/Name`), in file order.
fn ocg_objects(text: &str) -> Vec<(String, String)> {
    text.match_indices("/Type /OCG")
        .map(|(at, _)| {
            let head = &text[..at];
            let obj = head.rfind(" 0 obj").expect("object header");
            let id_start = head[..obj]
                .rfind(|c: char| !c.is_ascii_digit())
                .map_or(0, |i| i + 1);
            let id = &head[id_start..obj];
            let rest = &text[at..];
            let name_at = rest.find("/Name (").expect("group name") + "/Name (".len();
            let name_len = rest[name_at..].find(')').expect("name end");
            (
                format!("{id} 0 R"),
                rest[name_at..name_at + name_len].to_string(),
            )
        })
        .collect()
}

/// The references in the array that follows the first `key`.
fn refs_after(text: &str, key: &str) -> Vec<String> {
    let at = text.find(key).expect("key present") + key.len();
    let body = &text[at..];
    let open = body.find('[').expect("array open");
    let close = body.find(']').expect("array close");
    let tokens: Vec<&str> = body[open + 1..close].split_whitespace().collect();
    tokens.chunks(3).map(|c| c.join(" ")).collect()
}

fn name_of<'a>(groups: &'a [(String, String)], reference: &str) -> &'a str {
    groups
        .iter()
        .find(|(r, _)| r == reference)
        .map(|(_, name)| name.as_str())
        .expect("reference names a group")
}

#[test]
fn layers_write_one_named_group_each_in_list_order() {
    let text = export_text(&layered_ctx(), &layered(&[], true));
    assert!(text.contains("/OCProperties"), "catalog entry");
    let groups = ocg_objects(&text);
    let names: Vec<&str> = groups.iter().map(|(_, n)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "Wires and buses",
            "Signal harnesses",
            "Parts",
            "Net labels and ports",
            "Power ports",
            "Title block and border",
            "Watermark",
        ]
    );
    let order: Vec<&str> = refs_after(&text, "/Order")
        .iter()
        .map(|r| name_of(&groups, r))
        .collect();
    assert_eq!(
        order, names,
        "/Order lists the groups in PdfLayer::ALL order"
    );
    assert!(text.contains("/Properties"), "pages name the groups");
}

#[test]
fn layer_spans_merge_neighbours_of_one_group_and_skip_empty_runs() {
    use optional_content::LayerSpans;
    let mut spans = LayerSpans::default();
    spans.mark(PdfLayer::Parts, 2..4);
    spans.mark(PdfLayer::Parts, 4..6);
    spans.mark(PdfLayer::PowerPorts, 6..6);
    spans.mark(PdfLayer::PowerPorts, 6..7);
    spans.mark(PdfLayer::Parts, 7..8);
    assert_eq!(spans.starting_at(2), Some((PdfLayer::Parts, 6)));
    assert_eq!(spans.starting_at(4), None, "merged into the run at 2");
    assert_eq!(spans.starting_at(6), Some((PdfLayer::PowerPorts, 7)));
    assert_eq!(spans.starting_at(7), Some((PdfLayer::Parts, 8)));
    assert_eq!(spans.starting_at(0), None);
}

#[test]
fn group_names_match_the_layer_list() {
    let names: Vec<&str> = PdfLayer::ALL.iter().map(|l| l.group_name()).collect();
    assert_eq!(names.len(), 7);
    assert_eq!(names[0], "Wires and buses");
    assert_eq!(names[1], "Signal harnesses");
    assert_eq!(names[6], "Watermark");
}

/// The layer each element of `ctx`'s first page is drawn in.
fn element_layers(ctx: &ExportContext, opts: &PdfOptions) -> Vec<(SvgElement, Option<PdfLayer>)> {
    let (page_w_mm, page_h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    let (svg, spans) = SvgRenderContext::from_sheet_layered(
        &ctx.sheets[0],
        opts,
        page_w_mm,
        page_h_mm,
        MM_TO_PT,
        None,
    );
    let mut current: Option<(PdfLayer, usize)> = None;
    let mut out = Vec::with_capacity(svg.elements.len());
    for (index, element) in svg.elements.into_iter().enumerate() {
        if current.is_some_and(|(_, end)| end == index) {
            current = None;
        }
        if let Some(started) = spans.starting_at(index) {
            current = Some(started);
        }
        out.push((element, current.map(|(layer, _)| layer)));
    }
    out
}

/// Whether `element` is drawn in the harness colour or its soft tint.
fn is_harness_drawing(element: &SvgElement, palette: &SchematicPalette) -> bool {
    let harness = [palette.harness, palette.harness_soft];
    match element {
        SvgElement::Path { style, .. } => {
            style.stroke_rgb.is_some_and(|c| harness.contains(&c))
                || style.fill_rgb.is_some_and(|c| harness.contains(&c))
        }
        SvgElement::Text { fill_rgb, .. } => harness.contains(fill_rgb),
        SvgElement::Image { .. } => false,
    }
}

/// #2690: the harness wire, its connector, the entry tab and the names are
/// in the Signal harnesses group; Wires and buses keeps the wire and the
/// junction drawn after the harness, and holds no harness drawing.
#[test]
fn signal_harnesses_are_a_layer_of_their_own() {
    let opts = layered(&[], false);
    let drawn = element_layers(&layered_ctx(), &opts);
    let palette = &opts.palette;
    let harness: Vec<_> = drawn
        .iter()
        .filter(|(element, _)| is_harness_drawing(element, palette))
        .collect();
    // Wire, connector body, entry tab, entry name, type name.
    assert_eq!(harness.len(), 5, "{harness:#?}");
    assert!(
        harness
            .iter()
            .all(|(_, layer)| *layer == Some(PdfLayer::SignalHarnesses)),
        "{harness:#?}"
    );
    let wires: Vec<_> = drawn
        .iter()
        .filter(|(_, layer)| *layer == Some(PdfLayer::WiresAndBuses))
        .collect();
    assert_eq!(wires.len(), 2, "the wire and the junction: {wires:#?}");
    assert!(
        wires
            .iter()
            .all(|(element, _)| !is_harness_drawing(element, palette)),
        "{wires:#?}"
    );
    let in_harness_group = drawn
        .iter()
        .filter(|(_, layer)| *layer == Some(PdfLayer::SignalHarnesses))
        .count();
    assert_eq!(in_harness_group, 5, "nothing else is in the harness group");
}

/// #2690, in the written file: the layered PDF of a sheet with a harness
/// names the Signal harnesses group and wraps drawing in it.
#[test]
fn a_layered_pdf_of_a_harness_sheet_wraps_the_harness_in_its_group() {
    let ctx = layered_ctx();
    let opts = layered(&[PdfLayer::SignalHarnesses], false);
    let text = export_text(&ctx, &opts);
    let groups = ocg_objects(&text);
    let off: Vec<&str> = refs_after(&text, "/OFF")
        .iter()
        .map(|r| name_of(&groups, r))
        .collect();
    assert_eq!(off, ["Signal harnesses"], "it can open hidden on its own");
    let ops = page_ops(&ctx, &opts, 0);
    let begin = optional_content::begin_operator(PdfLayer::SignalHarnesses);
    assert_eq!(ops.matches(&begin).count(), 1, "one run: {ops}");
    let at = ops.find(&begin).expect("harness group");
    let run = &ops[at..at + ops[at..].find("EMC").expect("closed")];
    assert!(
        run.contains(" l\n"),
        "the harness wire is in the run: {run}"
    );
    assert!(
        run.contains("BT\n"),
        "the entry and type names are in the run: {run}"
    );
}

/// A sheet with no harness puts nothing in the harness group, and the wires
/// stay one run.
#[test]
fn a_sheet_without_a_harness_draws_nothing_in_the_harness_group() {
    let mut ctx = layered_ctx();
    ctx.sheets[0].schematic.harness = Default::default();
    let ops = page_ops(&ctx, &layered(&[], false), 0);
    let harness = optional_content::begin_operator(PdfLayer::SignalHarnesses);
    let wires = optional_content::begin_operator(PdfLayer::WiresAndBuses);
    assert!(!ops.contains(&harness), "{ops}");
    assert_eq!(ops.matches(&wires).count(), 1, "{ops}");
}

#[test]
fn off_lists_exactly_the_groups_hidden_on_open() {
    let hidden = [PdfLayer::Parts, PdfLayer::PowerPorts];
    let text = export_text(&layered_ctx(), &layered(&hidden, true));
    let groups = ocg_objects(&text);
    let off: Vec<&str> = refs_after(&text, "/OFF")
        .iter()
        .map(|r| name_of(&groups, r))
        .collect();
    assert_eq!(off, ["Parts", "Power ports"]);

    let text = export_text(&layered_ctx(), &layered(&[], true));
    assert!(refs_after(&text, "/OFF").is_empty(), "nothing hidden");
}

#[test]
fn the_watermark_group_always_prints() {
    let text = export_text(&layered_ctx(), &layered(&[PdfLayer::Watermark], true));
    let at = text.find("/Name (Watermark)").expect("watermark group");
    let object = &text[at..at + text[at..].find("endobj").expect("object end")];
    assert!(object.contains("/Usage"), "{object}");
    assert!(object.contains("/PrintState /ON"), "{object}");
    // The print event switches it on even though it opens hidden.
    assert!(text.contains("/Event /Print"), "auto state on print");
}

#[test]
fn no_watermark_writes_no_watermark_group() {
    let text = export_text(&layered_ctx(), &layered(&[], false));
    let names: Vec<String> = ocg_objects(&text).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names.len(), 6);
    assert!(!names.iter().any(|n| n == "Watermark"));
}

#[test]
fn each_kind_of_drawing_is_wrapped_in_balanced_marked_content() {
    let ops = page_ops(&layered_ctx(), &layered(&[], true), 0);
    let mut depth = 0i32;
    for line in ops.lines() {
        if line.ends_with(" BDC") {
            depth += 1;
            assert_eq!(depth, 1, "marked content never nests");
        } else if line == "EMC" {
            depth -= 1;
            assert!(depth >= 0, "EMC without BDC");
        }
    }
    assert_eq!(depth, 0, "every BDC is closed");
    for layer in PdfLayer::ALL {
        let begin = optional_content::begin_operator(layer);
        assert!(ops.contains(&begin), "{layer:?} is drawn in its group");
    }
}

/// Layers only wrap the drawing: with the marked content taken out, the
/// operators are those of the flat page.
#[test]
fn layers_change_no_drawing_operator() {
    let ctx = layered_ctx();
    let strip = |ops: String| -> Vec<String> {
        ops.lines()
            .filter(|l| !l.ends_with(" BDC") && *l != "EMC")
            .map(str::to_string)
            .collect()
    };
    let flat = strip(page_ops(&ctx, &PdfOptions::default(), 0));
    let wrapped = strip(page_ops(&ctx, &layered(&[], false), 0));
    assert_eq!(wrapped, flat);
}

#[test]
fn without_layers_the_pdf_stays_flat() {
    let ctx = layered_ctx();
    let text = export_text(&ctx, &PdfOptions::default());
    assert!(!text.contains("/OCProperties"));
    assert!(!text.contains("/Type /OCG"));
    assert!(!page_ops(&ctx, &PdfOptions::default(), 0).contains("BDC"));
    let explicit = PdfOptions {
        layers: None,
        watermark: None,
        ..PdfOptions::default()
    };
    let default_bytes = PdfExporter
        .export(&ctx, &PdfOptions::default())
        .expect("export")
        .bytes;
    let explicit_bytes = PdfExporter.export(&ctx, &explicit).expect("export").bytes;
    assert!(explicit_bytes == default_bytes, "None is today's flat PDF");
}
