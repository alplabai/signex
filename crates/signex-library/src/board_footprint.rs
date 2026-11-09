//! A library footprint as a board footprint (#663, #1312): the stored pads,
//! silkscreen, fab and courtyard, read as they are. The sketch solver never
//! runs here; the footprint is what its last bake stored.
//!
//! Nothing is guessed. A layer token, pad shape or drill the board cannot
//! hold exactly refuses the whole footprint with
//! [`FootprintResolveError::Unconvertible`]. Mapping rules:
//! `docs/architecture/library-footprint-bindings.md`, "Board footprints".

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::part::FootprintResolveError;
use signex_types::pcb::{self, DrillDef, PadType, Point};
use signex_types::pcb_layer::{CopperLayer, InnerCopper, PcbLayer, TechnicalLayer};
use signex_types::property::PcbProperty;
use uuid::Uuid;

use crate::primitive::footprint::{
    Drill, Footprint, FpGraphic, FpGraphicKind, LayerId, Pad, PadKind, PadShape,
};

/// The board form of `footprint`: top side, origin at (0, 0), rotation 0,
/// no link and no library identity (the resolver sets that). The footprint
/// and its pads get fresh uuids, since one library footprint is placed many
/// times. Reference and Value texts are created empty.
pub fn board_footprint(footprint: &Footprint) -> Result<pcb::Footprint, FootprintResolveError> {
    refuse_unheld_features(footprint)?;
    let pads = footprint
        .pads
        .iter()
        .map(board_pad)
        .collect::<Result<Vec<_>, _>>()?;
    let mut graphics = Vec::new();
    for (list, layer) in [
        (&footprint.silk_f, TechnicalLayer::SilkscreenTop),
        (&footprint.silk_b, TechnicalLayer::SilkscreenBottom),
        (&footprint.fab_f, TechnicalLayer::FabTop),
        (&footprint.fab_b, TechnicalLayer::FabBottom),
    ] {
        for graphic in list {
            graphics.push(board_graphic(graphic, layer)?);
        }
    }
    if !footprint.courtyard.points.is_empty() {
        graphics.push(courtyard(&footprint.courtyard.points));
    }
    Ok(pcb::Footprint {
        uuid: Uuid::now_v7(),
        reference: String::new(),
        value: String::new(),
        footprint_id: footprint.name.clone(),
        position: Point { x: 0.0, y: 0.0 },
        rotation: 0.0,
        layer: pcb_layer_token(PcbLayer::Copper(CopperLayer::Top)),
        locked: false,
        pads,
        graphics,
        properties: vec![
            text_property("Reference", TechnicalLayer::SilkscreenTop),
            text_property("Value", TechnicalLayer::FabTop),
        ],
        schematic_link: None,
        library_ref: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    })
}

fn unconvertible(what: &'static str, value: impl Into<String>) -> FootprintResolveError {
    FootprintResolveError::Unconvertible {
        what,
        value: value.into(),
    }
}

/// A board footprint holds pads, graphics and texts only. Copper pours,
/// keepouts, cutouts, v-scores and standalone mask or paste shapes would be
/// lost, so a footprint that has any is refused, not carried over without.
fn refuse_unheld_features(footprint: &Footprint) -> Result<(), FootprintResolveError> {
    let features = [
        ("footprint copper pours", footprint.pours.len()),
        ("footprint keepouts", footprint.keepouts.len()),
        ("footprint cutouts", footprint.cutouts.len()),
        ("footprint v-scores", footprint.v_scores.len()),
        ("footprint mask openings", footprint.mask_openings.len()),
        ("footprint mask exclusions", footprint.mask_excludes.len()),
        ("footprint paste apertures", footprint.paste_apertures.len()),
    ];
    match features.into_iter().find(|(_, count)| *count > 0) {
        Some((what, count)) => Err(unconvertible(what, count.to_string())),
        None => Ok(()),
    }
}

fn point(xy: [f64; 2]) -> Point {
    Point { x: xy[0], y: xy[1] }
}

fn board_pad(pad: &Pad) -> Result<pcb::Pad, FootprintResolveError> {
    let (shape, roundrect_ratio) = pad_shape(&pad.shape)?;
    let offset = (pad.copper_offset_x_mm, pad.copper_offset_y_mm);
    if offset.0.unwrap_or(0.0) != 0.0 || offset.1.unwrap_or(0.0) != 0.0 {
        return Err(unconvertible(
            "pad copper offset",
            format!("{:?}, {:?}", offset.0, offset.1),
        ));
    }
    Ok(pcb::Pad {
        uuid: Uuid::now_v7(),
        number: pad.number.clone(),
        pad_type: pad_type(pad.kind),
        shape,
        position: point(pad.position),
        size: point(pad.size),
        drill: pad.drill.as_ref().map(drill).transpose()?,
        layers: board_layers(&pad.layers)?,
        net: None,
        roundrect_ratio,
        rotation: pad.rotation,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    })
}

/// A plated half-hole is a plated hole on the board; a fiducial is a
/// surface pad whose layers say it has no paste.
fn pad_type(kind: PadKind) -> PadType {
    match kind {
        PadKind::Smd | PadKind::Fiducial => PadType::Smd,
        PadKind::Tht | PadKind::Castellated => PadType::Thru,
        PadKind::NptHole => PadType::NpThru,
        PadKind::ConnectorPad => PadType::Connect,
    }
}

fn pad_shape(shape: &PadShape) -> Result<(pcb::PadShape, f64), FootprintResolveError> {
    match shape {
        PadShape::Round => Ok((pcb::PadShape::Circle, 0.0)),
        PadShape::Rect => Ok((pcb::PadShape::Rect, 0.0)),
        PadShape::Oval => Ok((pcb::PadShape::Oval, 0.0)),
        PadShape::RoundRect { radius_ratio } if (0.0..=0.5).contains(radius_ratio) => {
            Ok((pcb::PadShape::RoundRect, *radius_ratio))
        }
        PadShape::RoundRect { radius_ratio } => {
            Err(unconvertible("pad corner ratio", radius_ratio.to_string()))
        }
        PadShape::Chamfered { .. } => Err(unconvertible("pad shape", "chamfered")),
        PadShape::Custom(_) => Err(unconvertible("pad shape", "custom")),
    }
}

fn drill(drill: &Drill) -> Result<DrillDef, FootprintResolveError> {
    if let Some(length) = drill.slot_length {
        return Err(unconvertible("slotted drill of length", length.to_string()));
    }
    Ok(DrillDef {
        diameter: drill.diameter,
        shape: String::new(),
    })
}

/// Every board layer the library tokens name, each once, in the order first
/// named.
fn board_layers(layers: &[LayerId]) -> Result<Vec<String>, FootprintResolveError> {
    let mut out: Vec<PcbLayer> = Vec::new();
    for token in layers {
        for layer in pcb_layers(token.as_str())? {
            if !out.contains(&layer) {
                out.push(layer);
            }
        }
    }
    Ok(out.into_iter().map(pcb_layer_token).collect())
}

/// The board layers one library layer token names. A board token
/// (`TopCopper`) reads as itself; the library editor's tokens (`F.Cu`,
/// `*.Mask`, `In2.Cu`) by the fixed table below; anything else is refused.
fn pcb_layers(token: &str) -> Result<Vec<PcbLayer>, FootprintResolveError> {
    use TechnicalLayer as T;
    if let Some(layer) = parse_pcb_layer(token) {
        return Ok(vec![layer]);
    }
    let copper = |c: CopperLayer| PcbLayer::Copper(c);
    let both = |top: T, bottom: T| vec![PcbLayer::Technical(top), PcbLayer::Technical(bottom)];
    let one = |t: T| vec![PcbLayer::Technical(t)];
    Ok(match token {
        "F.Cu" => vec![copper(CopperLayer::Top)],
        "B.Cu" => vec![copper(CopperLayer::Bottom)],
        "*.Cu" => vec![copper(CopperLayer::Top), copper(CopperLayer::Bottom)],
        "F.Mask" => one(T::SolderMaskTop),
        "B.Mask" => one(T::SolderMaskBottom),
        "*.Mask" => both(T::SolderMaskTop, T::SolderMaskBottom),
        "F.Paste" => one(T::PasteTop),
        "B.Paste" => one(T::PasteBottom),
        "*.Paste" => both(T::PasteTop, T::PasteBottom),
        "F.SilkS" => one(T::SilkscreenTop),
        "B.SilkS" => one(T::SilkscreenBottom),
        "F.Fab" => one(T::FabTop),
        "B.Fab" => one(T::FabBottom),
        "F.CrtYd" => one(T::CourtyardTop),
        "B.CrtYd" => one(T::CourtyardBottom),
        "F.Adhes" => one(T::AdhesiveTop),
        "B.Adhes" => one(T::AdhesiveBottom),
        "Edge.Cuts" => one(T::EdgeCuts),
        _ => match inner_copper(token) {
            Some(inner) => vec![copper(CopperLayer::Inner(inner))],
            None => return Err(unconvertible("footprint layer", token)),
        },
    })
}

/// `In<n>.Cu`, `n` written without sign or leading zero.
fn inner_copper(token: &str) -> Option<InnerCopper> {
    let digits = token.strip_prefix("In")?.strip_suffix(".Cu")?;
    if digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    InnerCopper::new(digits.parse().ok()?)
}

fn graphic(kind: &str, layer: TechnicalLayer, width: f64) -> pcb::FpGraphic {
    pcb::FpGraphic {
        graphic_type: kind.to_string(),
        layer: pcb_layer_token(PcbLayer::Technical(layer)),
        width,
        start: None,
        end: None,
        center: None,
        mid: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
        position: None,
        rotation: 0.0,
        fill: String::new(),
    }
}

fn board_graphic(
    source: &FpGraphic,
    layer: TechnicalLayer,
) -> Result<pcb::FpGraphic, FootprintResolveError> {
    let width = source.stroke_width;
    let mut out = match &source.kind {
        FpGraphicKind::Line { from, to } => pcb::FpGraphic {
            start: Some(point(*from)),
            end: Some(point(*to)),
            ..graphic("line", layer, width)
        },
        FpGraphicKind::Rectangle { from, to } => pcb::FpGraphic {
            start: Some(point(*from)),
            end: Some(point(*to)),
            ..graphic("rect", layer, width)
        },
        FpGraphicKind::Circle { center, radius } => pcb::FpGraphic {
            center: Some(point(*center)),
            radius: *radius,
            ..graphic("circle", layer, width)
        },
        FpGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => arc(*center, *radius, *start_deg, *end_deg, layer, width),
        FpGraphicKind::Text {
            position,
            content,
            size,
            frame,
        } => {
            if let Some((w, h)) = frame {
                return Err(unconvertible("framed footprint text", format!("{w} x {h}")));
            }
            pcb::FpGraphic {
                position: Some(point(*position)),
                text: content.clone(),
                font_size: *size,
                ..graphic("text", layer, width)
            }
        }
        FpGraphicKind::Polygon { vertices } => pcb::FpGraphic {
            points: vertices.iter().copied().map(point).collect(),
            ..graphic("poly", layer, width)
        },
    };
    if source.filled {
        out.fill = "solid".to_string();
    }
    Ok(out)
}

/// The arc's start, middle and end points, so the board keeps its sweep
/// direction (increasing angle, see `FpGraphicKind::Arc`).
fn arc(
    center: [f64; 2],
    radius: f64,
    start_deg: f64,
    end_deg: f64,
    layer: TechnicalLayer,
    width: f64,
) -> pcb::FpGraphic {
    let at = |deg: f64| {
        let (sin, cos) = deg.to_radians().sin_cos();
        Point {
            x: center[0] + radius * cos,
            y: center[1] + radius * sin,
        }
    };
    let sweep = FpGraphicKind::arc_sweep_deg(start_deg, end_deg);
    pcb::FpGraphic {
        center: Some(point(center)),
        radius,
        start: Some(at(start_deg)),
        mid: Some(at(start_deg + sweep / 2.0)),
        end: Some(at(start_deg + sweep)),
        ..graphic("arc", layer, width)
    }
}

fn courtyard(points: &[[f64; 2]]) -> pcb::FpGraphic {
    pcb::FpGraphic {
        points: points.iter().copied().map(point).collect(),
        ..graphic("poly", TechnicalLayer::CourtyardTop, 0.0)
    }
}

fn text_property(key: &str, layer: TechnicalLayer) -> PcbProperty {
    PcbProperty {
        key: key.to_string(),
        value: String::new(),
        position: None,
        rotation: 0.0,
        layer: Some(pcb_layer_token(PcbLayer::Technical(layer))),
        font_size: None,
        hidden: false,
    }
}

#[cfg(test)]
#[path = "board_footprint_tests.rs"]
mod tests;
