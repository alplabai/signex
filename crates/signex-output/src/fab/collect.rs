//! The board as Gerber images: every item on the image of its layer, in a
//! fixed order (zone fills, footprints, tracks, vias, board graphics, texts,
//! extra strokes, the profile, then the V-score lines), so one board gives
//! the same bytes.

use std::collections::BTreeMap;

use pcb_geom::{
    GeomError, Placement, PointNm, Polygon, Side, layer_bit, layer_mask, mm_to_nm, pad_layers,
    pad_world_center, pad_world_rotation_deg, via_layers,
};
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point, Segment, Via};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use super::draw::{Drawing, Drawn, Pen, draw};
use super::frame::{FabFrame, fab_angle_udeg, is_quarter_turn, swaps_axes};
use super::layers::{CopperStack, FabLayer, Face};
use super::marks::{ApertureShape, Attributes, Function, Mark, Step, doubled_area};
use super::notes::Tally;
use super::{FabExtras, FabStroke};

/// The pen the board profile is drawn with: 0.1 mm.
pub(crate) const PROFILE_WIDTH_NM: i64 = 100_000;

/// Every image's marks, ordered by image.
pub(crate) type Images = BTreeMap<FabLayer, Vec<Mark>>;

pub(crate) fn collect(
    board: &PcbBoard,
    extras: &FabExtras,
    stack: &CopperStack,
    tally: &mut Tally,
) -> Images {
    let mut images = Images::new();
    let frame = FabFrame::of(board);
    zone_fills(&mut images, extras, frame, tally);
    for footprint in &board.footprints {
        footprint_marks(&mut images, board, footprint, stack, frame, tally);
    }
    // Free pads (#2373) are marked as pads of the board's own frame.
    if !board.free.pads.is_empty() {
        let free = Footprint {
            pads: board.free.pads.clone(),
            ..signex_types::pcb_free::free_pad_frame()
        };
        footprint_marks(&mut images, board, &free, stack, frame, tally);
    }
    tracks(&mut images, board, frame, tally);
    arcs(&mut images, board, frame, tally);
    vias(&mut images, board, stack, frame, tally);
    board_graphics(&mut images, board, frame, tally);
    super::text::texts(&mut images, board, frame, tally);
    extra_strokes(&mut images, extras, frame, tally);
    profile(&mut images, board, frame, tally);
    v_scores(&mut images, board, frame, tally);
    images
}

/// The copper layers items are drawn on, for the stack.
pub(crate) fn used_copper(board: &PcbBoard, extras: &FabExtras) -> Vec<CopperLayer> {
    let tokens = board
        .segments
        .iter()
        .map(|s| s.layer.as_str())
        .chain(
            board
                .vias
                .iter()
                .flat_map(|v| v.layers.iter().map(String::as_str)),
        )
        .chain(board.footprints.iter().flat_map(|f| {
            f.pads
                .iter()
                .flat_map(|p| p.layers.iter().map(String::as_str))
        }))
        .chain(board.graphics.iter().map(|g| g.layer.as_str()))
        .chain(board.free.arcs.iter().map(|a| a.layer.as_str()))
        .chain(
            board
                .free
                .pads
                .iter()
                .flat_map(|p| p.layers.iter().map(String::as_str)),
        );
    let named = tokens
        .filter_map(signex_types::format::parse_pcb_layer)
        .filter_map(PcbLayer::copper);
    let extra = extras
        .zone_fills
        .iter()
        .map(|f| f.layer)
        .chain(extras.strokes.iter().map(|s| s.layer))
        .filter_map(PcbLayer::copper);
    named.chain(extra).collect()
}

fn push(images: &mut Images, layer: FabLayer, mark: Mark) {
    images.entry(layer).or_default().push(mark);
}

fn net_name(board: &PcbBoard, net: u32) -> Option<String> {
    if net == 0 {
        return None;
    }
    board
        .nets
        .iter()
        .find(|n| n.number == net)
        .map(|n| n.name.clone())
        .filter(|name| !name.is_empty())
}

fn copper_layers(stack: &CopperStack, mask: u32) -> impl Iterator<Item = CopperLayer> + '_ {
    stack
        .layers()
        .iter()
        .copied()
        .filter(move |l| mask & (1 << layer_bit(*l)) != 0)
}

/// Zone fills, largest outline first: each region's holes are cleared
/// right after it, and a fill inside another's hole is smaller, so it is
/// drawn after the clear and survives it. The regions of one net on one
/// layer are merged first: a hole of one that another covers is copper,
/// and clearing it would erase that copper.
fn zone_fills(images: &mut Images, extras: &FabExtras, frame: FabFrame, tally: &mut Tally) {
    let mut by_net: BTreeMap<(FabLayer, String), Vec<Polygon>> = BTreeMap::new();
    let mut regions = Vec::new();
    for (layer, net, polygon) in zone_polygons(extras, frame, tally) {
        if net.is_empty() {
            regions.push((layer, None, polygon));
        } else {
            by_net.entry((layer, net)).or_default().push(polygon);
        }
    }
    for ((layer, net), polygons) in by_net {
        let merged = if polygons.len() > 1 {
            pcb_geom::union(&polygons, &[])
        } else {
            polygons
        };
        regions.extend(merged.into_iter().map(|p| (layer, Some(net.clone()), p)));
    }
    regions.sort_by_key(|(_, _, p)| std::cmp::Reverse(doubled_area(&p.outer).abs()));
    for (layer, net, polygon) in regions {
        push(
            images,
            layer,
            Mark::Region {
                outline: polygon.outer,
                holes: polygon.holes,
                attributes: Attributes::net(net),
            },
        );
    }
}

/// Each zone fill of `extras` in the fab frame, with its layer and net
/// name; one that does not convert is tallied as skipped.
fn zone_polygons(
    extras: &FabExtras,
    frame: FabFrame,
    tally: &mut Tally,
) -> Vec<(FabLayer, String, Polygon)> {
    let mut regions = Vec::new();
    for fill in &extras.zone_fills {
        let Some(layer) = FabLayer::of(fill.layer) else {
            continue;
        };
        let ring = |points: &[Point]| -> Result<Vec<PointNm>, GeomError> {
            points
                .iter()
                .map(|p| frame.of_mm(*p, "zone fill"))
                .collect()
        };
        let built = ring(fill.outline.as_slice()).and_then(|outline| {
            let holes = fill
                .holes
                .iter()
                .map(|h| ring(h.as_slice()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((outline, holes))
        });
        match built {
            Ok((outline, holes)) if outline.len() >= 3 => {
                let holes = holes.into_iter().filter(|h| h.len() >= 3).collect();
                let polygon = Polygon::new(outline, holes);
                regions.push((layer, fill.net_name.clone(), polygon));
            }
            _ => tally.skipped += 1,
        }
    }
    regions
}

fn footprint_marks(
    images: &mut Images,
    board: &PcbBoard,
    footprint: &Footprint,
    stack: &CopperStack,
    frame: FabFrame,
    tally: &mut Tally,
) {
    for pad in &footprint.pads {
        if pad_marks(images, board, footprint, pad, stack, frame).is_err() {
            tally.skipped += 1;
        }
    }
    let Ok(placement) = Placement::of_footprint(footprint) else {
        tally.skipped += footprint.graphics.len();
        return;
    };
    let to_fab = |p: Point| -> Result<PointNm, GeomError> {
        Ok(frame.point(placement.to_world(PointNm::from_mm(p, "graphic")?)))
    };
    for graphic in &footprint.graphics {
        let Some(layer) = FabLayer::of_token(&graphic.layer) else {
            continue;
        };
        let pen = Pen {
            to_fab: &to_fab,
            arc_ccw_without_mid: placement.side == Side::Bottom,
            function: graphic_function(layer),
            width_override: (layer == FabLayer::Profile).then_some(PROFILE_WIDTH_NM),
        };
        place_drawing(images, layer, draw(&Drawing::from(graphic), &pen), tally);
    }
}

fn place_drawing(
    images: &mut Images,
    layer: FabLayer,
    drawn: Result<Drawn, GeomError>,
    tally: &mut Tally,
) {
    match drawn {
        Ok(Drawn::Marks(marks)) => marks.into_iter().for_each(|m| push(images, layer, m)),
        // `fab::text` writes it.
        Ok(Drawn::Text) => {}
        Ok(Drawn::Skipped) | Err(_) => tally.skipped += 1,
    }
}

pub(super) fn graphic_function(layer: FabLayer) -> Function {
    match layer {
        FabLayer::Profile => Function::Profile,
        FabLayer::Copper(_) => Function::NonConductor,
        _ => Function::Graphic,
    }
}

fn pad_function(pad: &Pad) -> Function {
    match pad.pad_type {
        PadType::Smd => Function::SmdPad,
        PadType::Thru => Function::ComponentPad,
        PadType::Connect => Function::ConnectorPad,
        PadType::NpThru => Function::NonPlatedPad,
    }
}

/// The copper layers a pad reaches. A non-plated pad has copper only on
/// the layers it lists.
fn pad_copper(footprint: &Footprint, pad: &Pad) -> u32 {
    match pad.pad_type {
        PadType::NpThru => layer_mask(pad.layers.iter().map(String::as_str)),
        _ => pad_layers(footprint, pad),
    }
}

/// The mask openings and paste of a pad: the mask and paste layers it
/// lists, or, when it lists none, its type's default (a surface pad opens
/// the mask and takes paste on its footprint's side, a connector pad only
/// opens the mask, a hole opens the mask on both faces).
pub(crate) fn pad_openings(footprint: &Footprint, pad: &Pad) -> Vec<FabLayer> {
    let listed: Vec<FabLayer> = pad
        .layers
        .iter()
        .filter_map(|t| FabLayer::of_token(t))
        .filter(|l| matches!(l, FabLayer::Mask(_) | FabLayer::Paste(_)))
        .collect();
    if !listed.is_empty() {
        return listed;
    }
    let side = match Side::of_layer(&footprint.layer) {
        Side::Top => Face::Top,
        Side::Bottom => Face::Bottom,
    };
    match pad.pad_type {
        PadType::Smd => vec![FabLayer::Mask(side), FabLayer::Paste(side)],
        PadType::Connect => vec![FabLayer::Mask(side)],
        PadType::Thru | PadType::NpThru => {
            vec![FabLayer::Mask(Face::Top), FabLayer::Mask(Face::Bottom)]
        }
    }
}

/// A pad's aperture at `size` (its size on the layer flashed, #2401) and
/// its centre in the fabrication frame.
pub(crate) fn pad_aperture(
    frame: FabFrame,
    footprint: &Footprint,
    pad: &Pad,
    size: Point,
) -> Result<(ApertureShape, PointNm), GeomError> {
    let at = frame.point(pad_world_center(footprint, pad)?);
    let udeg = fab_angle_udeg(pad_world_rotation_deg(footprint, pad)?);
    let w = mm_to_nm(size.x, "pad.size")?.abs();
    let h = mm_to_nm(size.y, "pad.size")?.abs();
    let upright = |w: i64, h: i64| if swaps_axes(udeg) { (h, w) } else { (w, h) };
    let shape = match pad.shape {
        PadShape::Circle => ApertureShape::Circle { diameter: w.min(h) },
        PadShape::Oval if w == h => ApertureShape::Circle { diameter: w },
        PadShape::Oval if is_quarter_turn(udeg) => {
            let (w, h) = upright(w, h);
            ApertureShape::Obround { w, h }
        }
        PadShape::Oval => ApertureShape::TurnedObround { w, h, udeg },
        PadShape::RoundRect if round_radius(pad, w, h) > 0 => ApertureShape::RoundRect {
            w,
            h,
            r: round_radius(pad, w, h),
            udeg,
        },
        PadShape::Rect | PadShape::RoundRect if is_quarter_turn(udeg) => {
            let (w, h) = upright(w, h);
            ApertureShape::Rect { w, h }
        }
        PadShape::Rect | PadShape::RoundRect => ApertureShape::TurnedRect { w, h, udeg },
    };
    Ok((shape, at))
}

/// The outer copper layer under a mask or paste opening: the one whose pad
/// size the opening follows.
fn face_copper(layer: FabLayer) -> CopperLayer {
    match layer {
        FabLayer::Mask(Face::Bottom) | FabLayer::Paste(Face::Bottom) => CopperLayer::Bottom,
        _ => CopperLayer::Top,
    }
}

/// A round-rect pad's corner radius in nm, as `pcb_geom::pad_shape` reads it.
fn round_radius(pad: &Pad, w: i64, h: i64) -> i64 {
    let short = w.min(h);
    ((pad.roundrect_ratio * short as f64).round() as i64).clamp(0, short / 2)
}

fn pad_marks(
    images: &mut Images,
    board: &PcbBoard,
    footprint: &Footprint,
    pad: &Pad,
    stack: &CopperStack,
    frame: FabFrame,
) -> Result<(), GeomError> {
    // Every size first, so a pad the geometry refuses is skipped whole.
    let flashes = copper_layers(stack, pad_copper(footprint, pad))
        .map(|copper| {
            Ok((
                copper,
                pad_aperture(frame, footprint, pad, pad.size_on(copper))?,
            ))
        })
        .collect::<Result<Vec<_>, GeomError>>()?;
    let openings = pad_openings(footprint, pad)
        .into_iter()
        .map(|layer| {
            let size = pad.size_on(face_copper(layer));
            Ok((layer, pad_aperture(frame, footprint, pad, size)?))
        })
        .collect::<Result<Vec<_>, GeomError>>()?;
    let function = pad_function(pad);
    let net = pad
        .net
        .as_ref()
        .map(|n| n.name.clone())
        .filter(|n| !n.is_empty())
        .or_else(|| pad.net.as_ref().and_then(|n| net_name(board, n.number)));
    let pin = (!footprint.reference.is_empty())
        .then(|| (footprint.reference.clone(), pad.number.clone()));
    for (copper, (shape, at)) in flashes {
        let attributes = Attributes {
            net: net.clone(),
            pin: pin.clone(),
        };
        push(
            images,
            FabLayer::Copper(copper),
            Mark::Flash {
                shape,
                function,
                at,
                attributes,
            },
        );
    }
    for (layer, (shape, at)) in openings {
        push(
            images,
            layer,
            Mark::Flash {
                shape,
                function: Function::PadOpening,
                at,
                attributes: Attributes::default(),
            },
        );
    }
    Ok(())
}

fn tracks(images: &mut Images, board: &PcbBoard, frame: FabFrame, tally: &mut Tally) {
    for segment in &board.segments {
        let Some(layer) = FabLayer::of_token(&segment.layer) else {
            continue;
        };
        match track_mark(board, segment, layer, frame) {
            Ok(mark) => push(images, layer, mark),
            Err(_) => tally.skipped += 1,
        }
    }
}

fn track_mark(
    board: &PcbBoard,
    segment: &Segment,
    layer: FabLayer,
    frame: FabFrame,
) -> Result<Mark, GeomError> {
    let start = frame.of_mm(segment.start, "track.start")?;
    let end = frame.of_mm(segment.end, "track.end")?;
    let width = mm_to_nm(segment.width, "track.width")?.abs();
    let (function, attributes) = if layer.is_copper() {
        let net = net_name(board, segment.net);
        (Function::Conductor, Attributes::net(net))
    } else {
        (graphic_function(layer), Attributes::default())
    };
    Ok(Mark::Stroke {
        width,
        function,
        start,
        steps: vec![Step::Line(end)],
        attributes,
    })
}

/// Free arcs (#2373) as strokes along their chords, within the error the
/// copper list reads them with: conductor of their net on copper, a
/// drawing on any other layer the images have.
fn arcs(images: &mut Images, board: &PcbBoard, frame: FabFrame, tally: &mut Tally) {
    for arc in &board.free.arcs {
        let Some(layer) = FabLayer::of_token(&arc.layer) else {
            continue;
        };
        let points = arc
            .points(pcb_geom::ARC_CHORD_ERROR_MM)
            .into_iter()
            .map(|p| frame.of_mm(p, "arc"))
            .collect::<Result<Vec<_>, _>>();
        let width = mm_to_nm(arc.width, "arc.width");
        let (Ok(points), Ok(width)) = (points, width) else {
            tally.skipped += 1;
            continue;
        };
        let Some((&start, rest)) = points.split_first() else {
            continue;
        };
        let (function, attributes) = if layer.is_copper() {
            (
                Function::Conductor,
                Attributes::net(net_name(board, arc.net)),
            )
        } else {
            (graphic_function(layer), Attributes::default())
        };
        push(
            images,
            layer,
            Mark::Stroke {
                width: width.abs(),
                function,
                start,
                steps: rest.iter().copied().map(Step::Line).collect(),
                attributes,
            },
        );
    }
}

fn vias(
    images: &mut Images,
    board: &PcbBoard,
    stack: &CopperStack,
    frame: FabFrame,
    tally: &mut Tally,
) {
    for via in &board.vias {
        let flashes = copper_layers(stack, via_layers(via))
            .map(|copper| Ok((copper, via_flash(via, copper, frame)?)))
            .collect::<Result<Vec<_>, GeomError>>();
        let Ok(flashes) = flashes else {
            tally.skipped += 1;
            continue;
        };
        let net = net_name(board, via.net);
        for (copper, (at, diameter)) in flashes {
            push(
                images,
                FabLayer::Copper(copper),
                Mark::Flash {
                    shape: ApertureShape::Circle { diameter },
                    function: Function::ViaPad,
                    at,
                    attributes: Attributes::net(net.clone()),
                },
            );
        }
    }
}

/// A via's centre in the fabrication frame and its pad diameter on
/// `copper` (#2401).
fn via_flash(via: &Via, copper: CopperLayer, frame: FabFrame) -> Result<(PointNm, i64), GeomError> {
    let at = frame.of_mm(via.position, "via.position")?;
    Ok((at, mm_to_nm(via.diameter_on(copper), "via.diameter")?.abs()))
}

fn board_graphics(images: &mut Images, board: &PcbBoard, frame: FabFrame, tally: &mut Tally) {
    let board_to_fab = |p: Point| frame.of_mm(p, "graphic");
    for graphic in &board.graphics {
        let Some(layer) = FabLayer::of_token(&graphic.layer) else {
            continue;
        };
        let pen = Pen {
            to_fab: &board_to_fab,
            arc_ccw_without_mid: false,
            function: graphic_function(layer),
            width_override: (layer == FabLayer::Profile).then_some(PROFILE_WIDTH_NM),
        };
        // A text frame's box; its lines are written by `fab::text`.
        place_drawing(images, layer, draw(&Drawing::from(graphic), &pen), tally);
    }
}

fn extra_strokes(images: &mut Images, extras: &FabExtras, frame: FabFrame, tally: &mut Tally) {
    for stroke in &extras.strokes {
        let Some(layer) = FabLayer::of(stroke.layer) else {
            continue;
        };
        match stroke_mark(stroke, layer, frame) {
            Ok(Some(mark)) => push(images, layer, mark),
            Ok(None) | Err(_) => tally.skipped += 1,
        }
    }
}

fn stroke_mark(
    stroke: &FabStroke,
    layer: FabLayer,
    frame: FabFrame,
) -> Result<Option<Mark>, GeomError> {
    let points = stroke
        .points
        .iter()
        .map(|p| frame.of_mm(*p, "stroke"))
        .collect::<Result<Vec<_>, _>>()?;
    let width = mm_to_nm(stroke.width_mm, "stroke.width")?.abs();
    let Some((&start, rest)) = points.split_first() else {
        return Ok(None);
    };
    Ok(Some(Mark::Stroke {
        width,
        function: graphic_function(layer),
        start,
        steps: rest.iter().copied().map(Step::Line).collect(),
        attributes: Attributes::default(),
    }))
}

/// The board outline and every through cutout, on the profile image.
fn profile(images: &mut Images, board: &PcbBoard, frame: FabFrame, tally: &mut Tally) {
    let rings = std::iter::once(board.outline.as_slice()).chain(
        board
            .sketch
            .iter()
            .flat_map(|s| s.cutouts.iter())
            .filter(|c| c.through)
            .map(|c| c.boundary.as_slice()),
    );
    for ring in rings {
        let points = ring
            .iter()
            .map(|p| frame.of_mm(*p, "profile"))
            .collect::<Result<Vec<_>, _>>();
        let mark = points
            .as_ref()
            .ok()
            .filter(|p| p.len() >= 3)
            .and_then(|p| Mark::closed_path(p, PROFILE_WIDTH_NM, Function::Profile));
        match mark {
            Some(mark) => push(images, FabLayer::Profile, mark),
            None if ring.is_empty() => {}
            None => tally.skipped += 1,
        }
    }
}

/// The board sketch's V-score lines (#2410), each a straight stroke with the
/// profile's pen on the V-cut image.
fn v_scores(images: &mut Images, board: &PcbBoard, frame: FabFrame, tally: &mut Tally) {
    for score in board.sketch.iter().flat_map(|s| s.v_scores.iter()) {
        match (
            frame.of_mm(score.from, "v-score"),
            frame.of_mm(score.to, "v-score"),
        ) {
            (Ok(start), Ok(end)) => push(
                images,
                FabLayer::VScore,
                Mark::Stroke {
                    width: PROFILE_WIDTH_NM,
                    function: Function::VScore,
                    start,
                    steps: vec![Step::Line(end)],
                    attributes: Attributes::default(),
                },
            ),
            _ => tally.skipped += 1,
        }
    }
}
