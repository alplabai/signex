//! The board as Gerber images: every item on the image of its layer, in a
//! fixed order (zone fills, footprints, tracks, vias, board graphics,
//! extra strokes, the profile, then the V-score lines), so one board gives
//! the same bytes.

use std::collections::BTreeMap;

use pcb_geom::{
    GeomError, Placement, PointNm, Side, layer_bit, layer_mask, mm_to_nm, pad_layers,
    pad_world_center, pad_world_rotation_deg, via_layers,
};
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point, Segment, Via};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use super::draw::{Drawing, Drawn, Pen, draw};
use super::frame::{fab_angle_udeg, fab_point, is_quarter_turn, swaps_axes};
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
    zone_fills(&mut images, extras, tally);
    for footprint in &board.footprints {
        footprint_marks(&mut images, board, footprint, stack, tally);
    }
    tracks(&mut images, board, tally);
    vias(&mut images, board, stack, tally);
    board_graphics(&mut images, board, tally);
    extra_strokes(&mut images, extras, tally);
    profile(&mut images, board, tally);
    v_scores(&mut images, board, tally);
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
        .chain(board.graphics.iter().map(|g| g.layer.as_str()));
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
/// drawn after the clear and survives it.
fn zone_fills(images: &mut Images, extras: &FabExtras, tally: &mut Tally) {
    let mut regions = Vec::new();
    for fill in &extras.zone_fills {
        let Some(layer) = FabLayer::of(fill.layer) else {
            continue;
        };
        let ring = |points: &[Point]| -> Result<Vec<PointNm>, GeomError> {
            points
                .iter()
                .map(|p| PointNm::from_mm(*p, "zone fill").map(fab_point))
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
                let area = doubled_area(&outline).abs();
                regions.push((area, layer, outline, holes, fill.net_name.clone()));
            }
            _ => tally.skipped += 1,
        }
    }
    regions.sort_by_key(|region| std::cmp::Reverse(region.0));
    for (_, layer, outline, holes, net) in regions {
        let holes = holes.into_iter().filter(|h| h.len() >= 3).collect();
        let net = Some(net).filter(|n| !n.is_empty());
        push(
            images,
            layer,
            Mark::Region {
                outline,
                holes,
                attributes: Attributes::net(net),
            },
        );
    }
}

fn footprint_marks(
    images: &mut Images,
    board: &PcbBoard,
    footprint: &Footprint,
    stack: &CopperStack,
    tally: &mut Tally,
) {
    for pad in &footprint.pads {
        if pad_marks(images, board, footprint, pad, stack).is_err() {
            tally.skipped += 1;
        }
    }
    let Ok(placement) = Placement::of_footprint(footprint) else {
        tally.skipped += footprint.graphics.len();
        return;
    };
    let to_fab = |p: Point| -> Result<PointNm, GeomError> {
        Ok(fab_point(
            placement.to_world(PointNm::from_mm(p, "graphic")?),
        ))
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
    tally.texts += footprint
        .properties
        .iter()
        .filter(|p| !p.hidden && !p.value.is_empty())
        .filter(|p| p.layer.as_deref().and_then(FabLayer::of_token).is_some())
        .count();
}

fn place_drawing(
    images: &mut Images,
    layer: FabLayer,
    drawn: Result<Drawn, GeomError>,
    tally: &mut Tally,
) {
    match drawn {
        Ok(Drawn::Marks(marks)) => marks.into_iter().for_each(|m| push(images, layer, m)),
        Ok(Drawn::Text) => tally.texts += 1,
        Ok(Drawn::Skipped) | Err(_) => tally.skipped += 1,
    }
}

fn graphic_function(layer: FabLayer) -> Function {
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

/// A pad's aperture and its centre in the fabrication frame.
pub(crate) fn pad_aperture(
    footprint: &Footprint,
    pad: &Pad,
) -> Result<(ApertureShape, PointNm), GeomError> {
    let at = fab_point(pad_world_center(footprint, pad)?);
    let udeg = fab_angle_udeg(pad_world_rotation_deg(footprint, pad)?);
    let w = mm_to_nm(pad.size.x, "pad.size")?.abs();
    let h = mm_to_nm(pad.size.y, "pad.size")?.abs();
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
) -> Result<(), GeomError> {
    let (shape, at) = pad_aperture(footprint, pad)?;
    let function = pad_function(pad);
    let net = pad
        .net
        .as_ref()
        .map(|n| n.name.clone())
        .filter(|n| !n.is_empty())
        .or_else(|| pad.net.as_ref().and_then(|n| net_name(board, n.number)));
    let pin = (!footprint.reference.is_empty())
        .then(|| (footprint.reference.clone(), pad.number.clone()));
    for copper in copper_layers(stack, pad_copper(footprint, pad)) {
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
    for layer in pad_openings(footprint, pad) {
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

fn tracks(images: &mut Images, board: &PcbBoard, tally: &mut Tally) {
    for segment in &board.segments {
        let Some(layer) = FabLayer::of_token(&segment.layer) else {
            continue;
        };
        match track_mark(board, segment, layer) {
            Ok(mark) => push(images, layer, mark),
            Err(_) => tally.skipped += 1,
        }
    }
}

fn track_mark(board: &PcbBoard, segment: &Segment, layer: FabLayer) -> Result<Mark, GeomError> {
    let start = fab_point(PointNm::from_mm(segment.start, "track.start")?);
    let end = fab_point(PointNm::from_mm(segment.end, "track.end")?);
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

fn vias(images: &mut Images, board: &PcbBoard, stack: &CopperStack, tally: &mut Tally) {
    for via in &board.vias {
        let Ok((at, diameter)) = via_flash(via) else {
            tally.skipped += 1;
            continue;
        };
        let net = net_name(board, via.net);
        for copper in copper_layers(stack, via_layers(via)) {
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

/// A via's centre in the fabrication frame and its pad diameter.
fn via_flash(via: &Via) -> Result<(PointNm, i64), GeomError> {
    let at = fab_point(PointNm::from_mm(via.position, "via.position")?);
    Ok((at, mm_to_nm(via.diameter, "via.diameter")?.abs()))
}

fn board_to_fab(p: Point) -> Result<PointNm, GeomError> {
    Ok(fab_point(PointNm::from_mm(p, "graphic")?))
}

fn board_graphics(images: &mut Images, board: &PcbBoard, tally: &mut Tally) {
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
        place_drawing(images, layer, draw(&Drawing::from(graphic), &pen), tally);
    }
    tally.texts += board
        .texts
        .iter()
        .filter(|t| !t.text.is_empty() && FabLayer::of_token(&t.layer).is_some())
        .count();
}

fn extra_strokes(images: &mut Images, extras: &FabExtras, tally: &mut Tally) {
    for stroke in &extras.strokes {
        let Some(layer) = FabLayer::of(stroke.layer) else {
            continue;
        };
        match stroke_mark(stroke, layer) {
            Ok(Some(mark)) => push(images, layer, mark),
            Ok(None) | Err(_) => tally.skipped += 1,
        }
    }
}

fn stroke_mark(stroke: &FabStroke, layer: FabLayer) -> Result<Option<Mark>, GeomError> {
    let points = stroke
        .points
        .iter()
        .map(|p| board_to_fab(*p))
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
fn profile(images: &mut Images, board: &PcbBoard, tally: &mut Tally) {
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
            .map(|p| board_to_fab(*p))
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
fn v_scores(images: &mut Images, board: &PcbBoard, tally: &mut Tally) {
    for score in board.sketch.iter().flat_map(|s| s.v_scores.iter()) {
        match (board_to_fab(score.from), board_to_fab(score.to)) {
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
