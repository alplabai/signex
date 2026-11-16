//! The drawing of one printed board page: the chosen sets' copper, pads,
//! vias, drawings and texts, then the drill holes, the outline and the
//! title block.

use pcb_geom::{GraphicShape, PlacedGraphic, PointNm, Polygon, ZoneFills, nm_to_mm};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use super::super::font::{PdfFont, best_alias_for_text, normalize_pdf_text, text_advance_pt};
use super::super::surface::PdfSurface;
use super::frame::{MARGIN_MM, TITLE_BLOCK_MM};
use super::{BoardLayerSet, BoardPrintOptions, PageFrame};
use crate::pdf::MM_TO_PT;

type Rgb = (f32, f32, f32);
type Pt = (f32, f32);

const BLACK: Rgb = (0.0, 0.0, 0.0);
const PAPER: Rgb = (1.0, 1.0, 1.0);
/// The thinnest line a page draws, so a hairline still prints.
const MIN_LINE_PT: f32 = 0.25;
/// The stroke of a pour's or rule area's outline, board mm.
const ZONE_EDGE_MM: f64 = 0.1;
/// Points per quarter turn of a round pad or via.
const ROUND_STEPS: usize = 8;
/// The sets in the order they are drawn: the top side last, on top.
const DRAW_ORDER: [BoardLayerSet; 4] = [
    BoardLayerSet::Mechanical,
    BoardLayerSet::Inner,
    BoardLayerSet::Bottom,
    BoardLayerSet::Top,
];

/// The ink of a set on white paper; black in monochrome.
fn ink(set: BoardLayerSet, monochrome: bool) -> Rgb {
    if monochrome {
        return BLACK;
    }
    match set {
        BoardLayerSet::Top => (0.78, 0.12, 0.12),
        BoardLayerSet::Bottom => (0.12, 0.27, 0.78),
        BoardLayerSet::Inner => (0.10, 0.52, 0.22),
        BoardLayerSet::Mechanical => (0.50, 0.10, 0.55),
    }
}

/// Draw `sets` of `board` on the page `frame` describes.
pub(super) fn page(
    s: &mut PdfSurface<'_>,
    board: &PcbBoard,
    fills: &ZoneFills,
    options: &BoardPrintOptions,
    frame: &PageFrame,
    sets: &[BoardLayerSet],
) {
    // Round caps and joins, so a bent track has no notch.
    s.raw_operator("1 J 1 j\n");
    for set in DRAW_ORDER.into_iter().filter(|set| sets.contains(set)) {
        draw_set(s, board, fills, frame, set, ink(set, options.monochrome));
    }
    let copper = sets.iter().any(|set| copper_mask(*set) != 0);
    if options.drill_marks && copper {
        drill_holes(s, board, frame);
    }
    if options.outline {
        let edge = ink(BoardLayerSet::Mechanical, options.monochrome);
        let ring: Vec<Pt> = board
            .outline
            .iter()
            .map(|p| frame.to_pt(p.x, p.y))
            .collect();
        stroke(s, &ring, true, frame.length_pt(0.1), edge);
    }
}

/// The copper layers of `set` as a `pcb_geom` layer mask.
fn copper_mask(set: BoardLayerSet) -> u32 {
    CopperLayer::all()
        .filter(|c| set.contains(PcbLayer::Copper(*c)))
        .fold(0, |mask, c| mask | (1 << pcb_geom::layer_bit(c)))
}

fn on_set(set: BoardLayerSet, layer: &str) -> bool {
    parse_pcb_layer(layer).is_some_and(|l| set.contains(l))
}

fn draw_set(
    s: &mut PdfSurface<'_>,
    board: &PcbBoard,
    fills: &ZoneFills,
    frame: &PageFrame,
    set: BoardLayerSet,
    rgb: Rgb,
) {
    let mm = |p: PointNm| frame.to_pt(nm_to_mm(p.x), nm_to_mm(p.y));
    draw_zones(s, board, fills, frame, set, rgb);
    for track in board.segments.iter().filter(|t| on_set(set, &t.layer)) {
        let ends = [
            frame.to_pt(track.start.x, track.start.y),
            frame.to_pt(track.end.x, track.end.y),
        ];
        stroke(s, &ends, false, frame.length_pt(track.width), rgb);
    }
    let mask = copper_mask(set);
    if mask != 0 {
        for fp in &board.footprints {
            for pad in &fp.pads {
                if pcb_geom::pad_layers(fp, pad) & mask == 0 {
                    continue;
                }
                if let Ok(shape) = pcb_geom::pad_shape(fp, pad) {
                    let ring: Vec<Pt> = shape.ring(ROUND_STEPS).into_iter().map(mm).collect();
                    fill(s, &[ring], rgb);
                }
            }
        }
        for via in &board.vias {
            if pcb_geom::via_layers(via) & mask != 0 {
                disc(
                    s,
                    frame,
                    (via.position.x, via.position.y),
                    via.diameter / 2.0,
                    rgb,
                );
            }
        }
    }
    for fp in &board.footprints {
        for graphic in fp.graphics.iter().filter(|g| on_set(set, &g.layer)) {
            if let Ok(Some(placed)) = pcb_geom::footprint_graphic_placement(fp, graphic) {
                let text = (graphic.text.as_str(), graphic.font_size);
                draw_graphic(s, frame, &placed, rgb, text);
            }
        }
    }
    for graphic in board.graphics.iter().filter(|g| on_set(set, &g.layer)) {
        if let Ok(Some(placed)) = pcb_geom::board_graphic_placement(graphic) {
            draw_graphic(s, frame, &placed, rgb, ("", 0.0));
        }
    }
    for text in board.texts.iter().filter(|t| on_set(set, &t.layer)) {
        if let Ok(placement) = pcb_geom::board_text_placement(text) {
            let at = placement.anchor;
            label(
                s,
                frame,
                (nm_to_mm(at.x), nm_to_mm(at.y)),
                placement.rotation_deg,
                &text.text,
                text.font_size,
                rgb,
            );
        }
    }
}

/// The zones of `set`, as the canvas draws them: a pour's computed copper
/// (clear of other nets) with its outline stroked, a rule area or keepout
/// as its outline alone, any other zone filled solid.
pub(super) fn draw_zones(
    s: &mut PdfSurface<'_>,
    board: &PcbBoard,
    fills: &ZoneFills,
    frame: &PageFrame,
    set: BoardLayerSet,
    rgb: Rgb,
) {
    let edge = frame.length_pt(ZONE_EDGE_MM);
    for (index, zone) in board.zones.iter().enumerate() {
        if !on_set(set, &zone.layer) {
            continue;
        }
        let outline: Vec<Pt> = zone.outline.iter().map(|p| frame.to_pt(p.x, p.y)).collect();
        if zone.is_rule_area() {
            stroke(s, &outline, true, edge, rgb);
        } else if zone.is_pour() {
            for piece in fills.of(index).map_or(&[][..], |f| f.pieces.as_slice()) {
                fill(s, &polygon_rings(frame, piece), rgb);
            }
            stroke(s, &outline, true, edge, rgb);
        } else {
            let mut rings: Vec<Vec<Pt>> = vec![outline];
            for hole in zone.usable_holes() {
                if let Some(hole) = zone.holes.get(hole) {
                    rings.push(hole.iter().map(|p| frame.to_pt(p.x, p.y)).collect());
                }
            }
            fill(s, &rings, rgb);
        }
    }
}

/// A computed copper area's outer ring and holes on the page.
fn polygon_rings(frame: &PageFrame, polygon: &Polygon) -> Vec<Vec<Pt>> {
    let ring = |points: &[PointNm]| -> Vec<Pt> {
        points
            .iter()
            .map(|p| frame.to_pt(nm_to_mm(p.x), nm_to_mm(p.y)))
            .collect()
    };
    std::iter::once(ring(&polygon.outer))
        .chain(polygon.holes.iter().map(|h| ring(h)))
        .collect()
}

/// One placed drawing; `text` is a text drawing's string and size in mm.
fn draw_graphic(
    s: &mut PdfSurface<'_>,
    frame: &PageFrame,
    placed: &PlacedGraphic,
    rgb: Rgb,
    text: (&str, f64),
) {
    let mm = |p: PointNm| frame.to_pt(nm_to_mm(p.x), nm_to_mm(p.y));
    let width = frame.length_pt(nm_to_mm(placed.width));
    match &placed.shape {
        GraphicShape::Line { a, b } => stroke(s, &[mm(*a), mm(*b)], false, width, rgb),
        GraphicShape::Arc {
            center,
            radius,
            start_deg,
            sweep_deg,
        } => {
            let points = arc_points(*center, *radius, *start_deg, *sweep_deg);
            let points: Vec<Pt> = points.into_iter().map(mm).collect();
            stroke(s, &points, false, width, rgb);
        }
        GraphicShape::Circle { center, radius } => {
            let points: Vec<Pt> = arc_points(*center, *radius, 0.0, 360.0)
                .into_iter()
                .map(mm)
                .collect();
            if placed.filled {
                fill(s, &[points], rgb);
            } else {
                stroke(s, &points, true, width, rgb);
            }
        }
        GraphicShape::Polygon { points } => {
            let points: Vec<Pt> = points.iter().copied().map(mm).collect();
            if placed.filled {
                fill(s, &[points], rgb);
            } else {
                stroke(s, &points, true, width, rgb);
            }
        }
        GraphicShape::Text(placement) => {
            let at = placement.anchor;
            let (string, size) = text;
            label(
                s,
                frame,
                (nm_to_mm(at.x), nm_to_mm(at.y)),
                placement.rotation_deg,
                string,
                size,
                rgb,
            );
        }
    }
}

/// Points along an arc in the model's axes, about one every 10 degrees.
fn arc_points(center: PointNm, radius: i64, start_deg: f64, sweep_deg: f64) -> Vec<PointNm> {
    let steps = ((sweep_deg.abs() / 10.0).ceil() as usize).clamp(2, 72);
    (0..=steps)
        .map(|i| {
            let t = (start_deg + sweep_deg * i as f64 / steps as f64).to_radians();
            PointNm::new(
                center.x + (radius as f64 * t.cos()).round() as i64,
                center.y + (radius as f64 * t.sin()).round() as i64,
            )
        })
        .collect()
}

/// Every pad and via hole as an open (paper) disc.
fn drill_holes(s: &mut PdfSurface<'_>, board: &PcbBoard, frame: &PageFrame) {
    for fp in &board.footprints {
        for pad in &fp.pads {
            let Some(drill) = pad.drill.as_ref().filter(|d| d.diameter > 0.0) else {
                continue;
            };
            if let Ok(center) = pcb_geom::pad_world_center(fp, pad) {
                let at = (nm_to_mm(center.x), nm_to_mm(center.y));
                disc(s, frame, at, drill.diameter / 2.0, PAPER);
            }
        }
    }
    for via in board.vias.iter().filter(|v| v.drill > 0.0) {
        disc(
            s,
            frame,
            (via.position.x, via.position.y),
            via.drill / 2.0,
            PAPER,
        );
    }
}

fn disc(s: &mut PdfSurface<'_>, frame: &PageFrame, at: (f64, f64), radius_mm: f64, rgb: Rgb) {
    if !(radius_mm.is_finite() && radius_mm > 0.0) {
        return;
    }
    let ring: Vec<Pt> = (0..36)
        .map(|i| {
            let t = (i as f64 * 10.0).to_radians();
            frame.to_pt(at.0 + radius_mm * t.cos(), at.1 + radius_mm * t.sin())
        })
        .collect();
    fill(s, &[ring], rgb);
}

/// Fill `rings` as one even-odd area: an outline and its holes.
fn fill(s: &mut PdfSurface<'_>, rings: &[Vec<Pt>], rgb: Rgb) {
    let rings: Vec<&Vec<Pt>> = rings.iter().filter(|r| r.len() >= 3).collect();
    if rings.is_empty() {
        return;
    }
    s.set_fill_color(rgb.0, rgb.1, rgb.2);
    for ring in rings {
        path(s, ring, true);
    }
    s.raw_operator("f*\n");
}

fn stroke(s: &mut PdfSurface<'_>, points: &[Pt], closed: bool, width_pt: f32, rgb: Rgb) {
    if points.len() < 2 {
        return;
    }
    s.set_stroke_color(rgb.0, rgb.1, rgb.2);
    s.set_stroke_width(width_pt.max(MIN_LINE_PT));
    path(s, points, closed);
    s.raw_operator("S\n");
}

fn path(s: &mut PdfSurface<'_>, points: &[Pt], closed: bool) {
    for (i, (x, y)) in points.iter().enumerate() {
        let op = if i == 0 { "m" } else { "l" };
        s.raw_operator(&format!("{x} {y} {op}\n"));
    }
    if closed {
        s.raw_operator("h\n");
    }
}

/// A text centred on `at` (model mm), `size_mm` high, turned by
/// `rotation_deg`. Too small to read, it is left out.
fn label(
    s: &mut PdfSurface<'_>,
    frame: &PageFrame,
    at: (f64, f64),
    rotation_deg: f64,
    text: &str,
    size_mm: f64,
    rgb: Rgb,
) {
    let size = frame.length_pt(size_mm);
    let text = normalize_pdf_text(text);
    if text.trim().is_empty() || !(size.is_finite() && size >= 1.0) {
        return;
    }
    let alias = best_alias_for_text(PdfFont::RobotoRegular.alias(), &text);
    // Model y runs down the page, so a turn reads the other way round
    // unless the page is mirrored as well.
    let turn = if frame.mirrored() {
        rotation_deg
    } else {
        -rotation_deg
    };
    let (sin, cos) = (
        turn.to_radians().sin() as f32,
        turn.to_radians().cos() as f32,
    );
    let half_w = text_advance_pt(alias, &text, size) / 2.0;
    let rise = size * 0.35;
    let (x, y) = frame.to_pt(at.0, at.1);
    let start = (x - half_w * cos + rise * sin, y - half_w * sin - rise * cos);
    s.set_fill_color(rgb.0, rgb.1, rgb.2);
    s.text_at_rotated(start.0, start.1, alias, size, &text, turn as f32);
}

/// What a page's title block says.
pub(super) struct TitleBlock<'a> {
    pub title: &'a str,
    pub label: &'a str,
    /// Page mm per board mm.
    pub scale: f64,
    /// This page and the page count.
    pub page: (usize, usize),
}

/// The title block: a framed strip under the drawing with the board name,
/// the page's sets, the scale and the page number.
pub(super) fn title_block(
    s: &mut PdfSurface<'_>,
    frame: &PageFrame,
    block: &TitleBlock<'_>,
    monochrome: bool,
) {
    let (page_w, _) = frame.page_pt();
    let margin = (MARGIN_MM * MM_TO_PT) as f32;
    let height = (TITLE_BLOCK_MM * MM_TO_PT) as f32;
    let (left, right) = (margin, page_w - margin);
    let (bottom, top) = (margin, margin + height);
    let line = if monochrome {
        BLACK
    } else {
        (0.25, 0.25, 0.25)
    };
    let rect = [(left, bottom), (right, bottom), (right, top), (left, top)];
    stroke(s, &rect, true, 0.6, line);
    s.set_fill_color(0.0, 0.0, 0.0);
    let bold = PdfFont::RobotoBold.alias();
    let regular = PdfFont::RobotoRegular.alias();
    let title = normalize_pdf_text(block.title);
    let label = normalize_pdf_text(block.label);
    let pad = 6.0;
    s.text_at(
        left + pad,
        top - 15.0,
        best_alias_for_text(bold, &title),
        11.0,
        &title,
    );
    s.text_at(
        left + pad,
        bottom + 9.0,
        best_alias_for_text(regular, &label),
        9.0,
        &label,
    );
    let scale = scale_text(block.scale);
    let page = format!("{} / {}", block.page.0, block.page.1);
    for (text, y) in [(scale, top - 15.0), (page, bottom + 9.0)] {
        let w = text_advance_pt(regular, &text, 9.0);
        s.text_at(right - pad - w, y, regular, 9.0, &text);
    }
}

/// `1:1`, `1:2.5` for a reduction, `2:1` for an enlargement.
pub(super) fn scale_text(scale: f64) -> String {
    let short = |v: f64| {
        let text = format!("{v:.2}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    if !(scale.is_finite() && scale > 0.0) {
        return "-".to_string();
    }
    if scale >= 1.0 {
        format!("{}:1", short(scale))
    } else {
        format!("1:{}", short(1.0 / scale))
    }
}
