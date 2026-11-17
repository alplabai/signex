//! The facts and edits of free arcs, free pads, strings and dimensions
//! (#2373). Positions and angles are shown from the board origin with Y up
//! (#2384) and turned back into the model's here; the commands come from
//! `signex_engine::pcb::primitive_edit`.

use pcb_geom::ItemRef;
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::primitive_edit::{
    ArcEdit, ArcSpan, DimensionEdit, PrimitiveEditRefusal, TextEdit, arc_edit_command,
    dimension_edit_command, free_pad_edit_command, text_edit_command,
};
use signex_i18n::I18n;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{BoardText, Footprint, Pad, PcbBoard};
use signex_types::pcb_arc::TrackArc;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_free::free_pad_frame;
use signex_types::pcb_layer::{CopperLayer, MechanicalLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::PrimitiveEdit;
use crate::app::Signex;
use crate::app::handlers::pcb_tool::precision_of;
use crate::panels::pcb_primitive_properties::{
    ArcFacts, AreaLayerChoice, DimensionFacts, Editability, PcbPrimitiveFacts, PrimitiveCommon,
    PrimitiveField as F, PrimitiveTarget, TextFacts, shared,
};
use crate::pcb_canvas::layer_style;

/// The board's layers that `keep` takes, as dropdown choices. A board that
/// lists no layers offers the outer copper, the overlays, the Dimensions
/// layer and the first mechanical layer.
fn layer_choices(
    board: &PcbBoard,
    i18n: &I18n,
    keep: impl Fn(PcbLayer) -> bool,
) -> Vec<AreaLayerChoice> {
    let listed: Vec<PcbLayer> = board
        .layers
        .iter()
        .filter_map(|l| parse_pcb_layer(&l.name))
        .collect();
    let layers = if listed.is_empty() {
        let mut fallback = vec![
            PcbLayer::Copper(CopperLayer::Top),
            PcbLayer::Copper(CopperLayer::Bottom),
            PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
            PcbLayer::Technical(TechnicalLayer::SilkscreenBottom),
            PcbLayer::Technical(TechnicalLayer::Dimensions),
        ];
        fallback.extend(MechanicalLayer::new(1).map(PcbLayer::Mechanical));
        fallback
    } else {
        listed
    };
    layers
        .into_iter()
        .filter(|l| keep(*l))
        .map(|layer| AreaLayerChoice {
            layer,
            label: layer_style::layer_name(i18n, layer).into_string(),
        })
        .collect()
}

fn layer_choice(token: Option<&str>, i18n: &I18n) -> Option<AreaLayerChoice> {
    let layer = parse_pcb_layer(token?)?;
    Some(AreaLayerChoice {
        layer,
        label: layer_style::layer_name(i18n, layer).into_string(),
    })
}

/// An angle of the model as the user reads it: Y up turns it the other way.
fn user_angle(model_deg: f64) -> f64 {
    (-model_deg).rem_euclid(360.0)
}

pub(super) fn arc_facts(
    board: &PcbBoard,
    common: PrimitiveCommon,
    arcs: &[&TrackArc],
    i18n: &I18n,
    mode: Option<String>,
) -> ArcFacts {
    let origin = board.options.origin;
    let spans: Vec<Option<ArcSpan>> = arcs.iter().map(|a| ArcSpan::of(a)).collect();
    // Model angles rise clockwise on screen; read with Y up the arc runs
    // counter-clockwise from the model's `to` to its `from`.
    let span =
        |f: &dyn Fn(&ArcSpan) -> f64| shared(spans.iter().map(|s| s.as_ref().map(f))).flatten();
    ArcFacts {
        is_copper: arcs.iter().all(|a| pcb_geom::arc_is_copper(a)),
        layer: layer_choice(shared(arcs.iter().map(|a| a.layer.as_str())), i18n),
        layers: layer_choices(board, i18n, |_| true),
        center: (
            span(&|s: &ArcSpan| origin.user_x(s.center.x)),
            span(&|s: &ArcSpan| origin.user_y(s.center.y)),
        ),
        radius: span(&|s: &ArcSpan| s.radius),
        start_angle: span(&|s: &ArcSpan| user_angle(s.to_deg)),
        end_angle: span(&|s: &ArcSpan| user_angle(s.from_deg)),
        width: shared(arcs.iter().map(|a| a.width)),
        length_mm: Some(arcs.iter().map(|a| a.length_mm()).sum()),
        mode,
        common,
    }
}

pub(super) fn text_facts(
    board: &PcbBoard,
    common: PrimitiveCommon,
    texts: &[&BoardText],
    i18n: &I18n,
) -> TextFacts {
    let origin = board.options.origin;
    TextFacts {
        text: shared(texts.iter().map(|t| t.text.clone())),
        x: shared(texts.iter().map(|t| origin.user_x(t.position.x))),
        y: shared(texts.iter().map(|t| origin.user_y(t.position.y))),
        height: shared(texts.iter().map(|t| t.font_size)),
        rotation: shared(texts.iter().map(|t| t.rotation)),
        layer: layer_choice(shared(texts.iter().map(|t| t.layer.as_str())), i18n),
        layers: layer_choices(board, i18n, PcbLayer::takes_text),
        common,
    }
}

pub(super) fn dimension_facts(
    board: &PcbBoard,
    common: PrimitiveCommon,
    dimensions: &[&LinearDimension],
    i18n: &I18n,
) -> DimensionFacts {
    let origin = board.options.origin;
    DimensionFacts {
        start: (
            shared(dimensions.iter().map(|d| origin.user_x(d.start.x))),
            shared(dimensions.iter().map(|d| origin.user_y(d.start.y))),
        ),
        end: (
            shared(dimensions.iter().map(|d| origin.user_x(d.end.x))),
            shared(dimensions.iter().map(|d| origin.user_y(d.end.y))),
        ),
        offset: shared(dimensions.iter().map(|d| d.offset)),
        line_width: shared(dimensions.iter().map(|d| d.line_width)),
        text_height: shared(dimensions.iter().map(|d| d.text_height)),
        precision: shared(dimensions.iter().map(|d| d.precision)),
        value: shared(dimensions.iter().map(|d| d.value_text())),
        layer: layer_choice(shared(dimensions.iter().map(|d| d.layer.as_str())), i18n),
        layers: layer_choices(board, i18n, PcbLayer::takes_text),
        common,
    }
}

impl Signex {
    /// The Properties facts of the strings, arcs, free pads or dimensions in
    /// `target`, all of one kind; `None` when one of them is gone.
    pub(super) fn free_primitive_facts(
        &self,
        board: &PcbBoard,
        target: PrimitiveTarget,
    ) -> Option<PcbPrimitiveFacts> {
        let i18n = &*self.ui_state.i18n;
        let editable = Editability::Editable;
        let first = *target.items.first()?;
        let facts = match first {
            ItemRef::Arc(_) => {
                let arcs: Vec<&TrackArc> = target
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ItemRef::Arc(id) => board.free.arcs.iter().find(|a| a.uuid == *id),
                        _ => None,
                    })
                    .collect();
                let common = self.primitive_common(board, target, editable)?;
                PcbPrimitiveFacts::Arc(arc_facts(board, common, &arcs, i18n, None))
            }
            ItemRef::Text(_) => {
                let texts: Vec<&BoardText> = target
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ItemRef::Text(id) => board.texts.iter().find(|t| t.uuid == *id),
                        _ => None,
                    })
                    .collect();
                let common = self.primitive_common(board, target, editable)?;
                PcbPrimitiveFacts::Text(text_facts(board, common, &texts, i18n))
            }
            ItemRef::Dimension(_) => {
                let dimensions: Vec<&LinearDimension> = target
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ItemRef::Dimension(id) => {
                            board.free.dimensions.iter().find(|d| d.uuid == *id)
                        }
                        _ => None,
                    })
                    .collect();
                let common = self.primitive_common(board, target, editable)?;
                PcbPrimitiveFacts::Dimension(dimension_facts(board, common, &dimensions, i18n))
            }
            ItemRef::FreePad(_) => {
                let frame = free_pad_frame();
                let pads: Vec<(&Footprint, &Pad)> = target
                    .items
                    .iter()
                    .filter_map(|i| match i {
                        ItemRef::FreePad(id) => board.free.pads.iter().find(|p| p.uuid == *id),
                        _ => None,
                    })
                    .map(|pad| (&frame, pad))
                    .collect();
                let common = self.primitive_common(board, target, editable)?;
                let origin = board.options.origin;
                PcbPrimitiveFacts::Pad(super::facts::pad_facts(
                    origin,
                    common,
                    &pads,
                    &signex_renderer::pcb::board_copper_layers(&board.layers),
                    i18n,
                ))
            }
            _ => return None,
        };
        Some(facts)
    }
}

/// The ids of the items of one kind that `pick` names.
fn ids_of(items: &[ItemRef], pick: impl Fn(&ItemRef) -> Option<Uuid>) -> Vec<Uuid> {
    items.iter().filter_map(pick).collect()
}

/// The command for `edit` on the selected arcs, strings, free pads or
/// dimensions. X and Y were turned into the model by `in_board`; an angle
/// typed with Y up is turned here.
pub(super) fn free_command(
    board: &PcbBoard,
    items: &[ItemRef],
    edit: &PrimitiveEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    match items.first() {
        Some(ItemRef::Arc(_)) => {
            let ids = ids_of(items, |i| match i {
                ItemRef::Arc(id) => Some(*id),
                _ => None,
            });
            match arc_edit(edit) {
                Some(e) => arc_edit_command(board, &ids, e),
                None => Ok(None),
            }
        }
        Some(ItemRef::Text(_)) => {
            let ids = ids_of(items, |i| match i {
                ItemRef::Text(id) => Some(*id),
                _ => None,
            });
            match text_edit(edit) {
                Some(e) => text_edit_command(board, &ids, &e),
                None => Ok(None),
            }
        }
        Some(ItemRef::FreePad(_)) => {
            let ids = ids_of(items, |i| match i {
                ItemRef::FreePad(id) => Some(*id),
                _ => None,
            });
            match super::pad_edit(edit) {
                Some(e) => free_pad_edit_command(board, &ids, &e),
                None => Ok(None),
            }
        }
        Some(ItemRef::Dimension(_)) => {
            let ids = ids_of(items, |i| match i {
                ItemRef::Dimension(id) => Some(*id),
                _ => None,
            });
            match dimension_edit(edit) {
                Some(e) => dimension_edit_command(board, &ids, e?),
                None => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

fn arc_edit(edit: &PrimitiveEdit) -> Option<ArcEdit> {
    Some(match *edit {
        PrimitiveEdit::Number(F::X, v) => ArcEdit::CenterX(v),
        PrimitiveEdit::Number(F::Y, v) => ArcEdit::CenterY(v),
        PrimitiveEdit::Number(F::Radius, v) => ArcEdit::Radius(v),
        // Read with Y up, the start angle is the model's `to`, the end its
        // `from`.
        PrimitiveEdit::Number(F::StartAngle, v) => ArcEdit::ToAngle(-v),
        PrimitiveEdit::Number(F::EndAngle, v) => ArcEdit::FromAngle(-v),
        PrimitiveEdit::Number(F::Width, v) => ArcEdit::Width(v),
        PrimitiveEdit::AreaLayer(layer) => ArcEdit::Layer(layer),
        PrimitiveEdit::Layer(layer) => ArcEdit::Layer(PcbLayer::Copper(layer)),
        PrimitiveEdit::Net(net) => ArcEdit::Net(net),
        _ => return None,
    })
}

fn text_edit(edit: &PrimitiveEdit) -> Option<TextEdit> {
    Some(match edit {
        PrimitiveEdit::Text(words) => TextEdit::Text(words.clone()),
        PrimitiveEdit::Number(F::X, v) => TextEdit::X(*v),
        PrimitiveEdit::Number(F::Y, v) => TextEdit::Y(*v),
        PrimitiveEdit::Number(F::TextHeight, v) => TextEdit::Height(*v),
        PrimitiveEdit::Number(F::Rotation, v) => TextEdit::Rotation(*v),
        PrimitiveEdit::AreaLayer(layer) => TextEdit::Layer(*layer),
        _ => return None,
    })
}

/// A typed precision that is not a whole number of decimals the dimension
/// can show is refused as out of range.
fn dimension_edit(edit: &PrimitiveEdit) -> Option<Result<DimensionEdit, PrimitiveEditRefusal>> {
    Some(Ok(match *edit {
        PrimitiveEdit::Number(F::StartX, v) => DimensionEdit::StartX(v),
        PrimitiveEdit::Number(F::StartY, v) => DimensionEdit::StartY(v),
        PrimitiveEdit::Number(F::EndX, v) => DimensionEdit::EndX(v),
        PrimitiveEdit::Number(F::EndY, v) => DimensionEdit::EndY(v),
        PrimitiveEdit::Number(F::Offset, v) => DimensionEdit::Offset(v),
        PrimitiveEdit::Number(F::LineWidth, v) => DimensionEdit::LineWidth(v),
        PrimitiveEdit::Number(F::TextHeight, v) => DimensionEdit::TextHeight(v),
        PrimitiveEdit::Number(F::Precision, v) => match precision_of(v) {
            Some(p) => DimensionEdit::Precision(p),
            None => return Some(Err(PrimitiveEditRefusal::OutOfRange)),
        },
        PrimitiveEdit::AreaLayer(layer) => DimensionEdit::Layer(layer),
        _ => return None,
    }))
}
