//! The facts and edits of the drawing panel (#2424): board lines, arcs,
//! circles, rectangles and text frames, read from the board's drawings. The
//! edits are `signex_engine::pcb::primitive_edit::GraphicEdit`, one
//! replace per field.

use pcb_geom::ItemRef;
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::primitive_edit::{GraphicEdit, PrimitiveEditRefusal, graphic_edit_command};
use signex_i18n::I18n;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{BoardGraphic, PcbBoard};
use signex_types::pcb_graphic::{takes_board_graphics, text_to_field};
use signex_types::pcb_layer::PcbLayer;

use super::PrimitiveEdit;
use crate::panels::pcb_primitive_properties::{
    AreaLayerChoice, GraphicFacts, PrimitiveCommon, PrimitiveField, shared,
};
use crate::pcb_canvas::layer_style;

/// The layers a drawing can be on: every technical layer but the keep-outs,
/// then the mechanical layers the board's layer table lists.
fn graphic_layers(board: &PcbBoard, i18n: &I18n) -> Vec<AreaLayerChoice> {
    let mechanical = board
        .layers
        .iter()
        .filter_map(|def| def.pcb_layer())
        .filter(|layer| matches!(layer, PcbLayer::Mechanical(_)));
    PcbLayer::all()
        .filter(|layer| matches!(layer, PcbLayer::Technical(_)))
        .chain(mechanical)
        .filter(|layer| takes_board_graphics(*layer))
        .map(|layer| AreaLayerChoice {
            layer,
            label: layer_style::layer_name(i18n, layer).into_string(),
        })
        .collect()
}

/// The facts of `graphics`, all on `board`.
pub(super) fn graphic_facts(
    board: &PcbBoard,
    common: PrimitiveCommon,
    graphics: &[&BoardGraphic],
    i18n: &I18n,
) -> GraphicFacts {
    let origin = board.options.origin;
    let x = |p: Option<signex_types::pcb::Point>| p.map(|p| origin.user_x(p.x));
    let y = |p: Option<signex_types::pcb::Point>| p.map(|p| origin.user_y(p.y));
    let layer = shared(graphics.iter().map(|g| g.layer.as_str()))
        .and_then(parse_pcb_layer)
        .map(|layer| AreaLayerChoice {
            layer,
            label: layer_style::layer_name(i18n, layer).into_string(),
        });
    GraphicFacts {
        kind: shared(graphics.iter().map(|g| g.kind())).flatten(),
        layer,
        layers: graphic_layers(board, i18n),
        width: shared(graphics.iter().map(|g| g.width)),
        start: (
            shared(graphics.iter().map(|g| x(g.start))).flatten(),
            shared(graphics.iter().map(|g| y(g.start))).flatten(),
        ),
        end: (
            shared(graphics.iter().map(|g| x(g.end))).flatten(),
            shared(graphics.iter().map(|g| y(g.end))).flatten(),
        ),
        center: (
            shared(graphics.iter().map(|g| x(g.center))).flatten(),
            shared(graphics.iter().map(|g| y(g.center))).flatten(),
        ),
        radius: shared(graphics.iter().map(|g| g.radius)),
        text: shared(graphics.iter().map(|g| text_to_field(&g.text))),
        text_height: shared(graphics.iter().map(|g| pcb_geom::text_frame_size_mm(g))),
        common,
    }
}

/// The drawing edit `edit` stands for; `None` for a field drawings do not
/// have.
pub(crate) fn graphic_edit(edit: &PrimitiveEdit) -> Option<GraphicEdit> {
    use PrimitiveField as F;
    Some(match edit {
        PrimitiveEdit::AreaLayer(layer) => GraphicEdit::Layer(*layer),
        PrimitiveEdit::Layer(layer) => GraphicEdit::Layer(PcbLayer::Copper(*layer)),
        PrimitiveEdit::Number(F::Width, v) => GraphicEdit::Width(*v),
        PrimitiveEdit::Number(F::StartX, v) => GraphicEdit::StartX(*v),
        PrimitiveEdit::Number(F::StartY, v) => GraphicEdit::StartY(*v),
        PrimitiveEdit::Number(F::EndX, v) => GraphicEdit::EndX(*v),
        PrimitiveEdit::Number(F::EndY, v) => GraphicEdit::EndY(*v),
        PrimitiveEdit::Number(F::CenterX, v) => GraphicEdit::CenterX(*v),
        PrimitiveEdit::Number(F::CenterY, v) => GraphicEdit::CenterY(*v),
        PrimitiveEdit::Number(F::Radius, v) => GraphicEdit::Radius(*v),
        PrimitiveEdit::Number(F::TextHeight, v) => GraphicEdit::TextHeight(*v),
        PrimitiveEdit::Text(text) => GraphicEdit::Text(text.clone()),
        _ => return None,
    })
}

/// The command for `edit` on the selected drawings `items`.
pub(super) fn graphic_command(
    board: &PcbBoard,
    items: &[ItemRef],
    edit: &PrimitiveEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let ids: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            ItemRef::Graphic { graphic, .. } => Some(*graphic),
            _ => None,
        })
        .collect();
    match graphic_edit(edit) {
        Some(e) => graphic_edit_command(board, &ids, &e),
        None => Ok(None),
    }
}
