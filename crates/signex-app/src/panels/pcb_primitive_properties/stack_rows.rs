//! The rows a via span and per-layer sizes add to the via and pad panels
//! (#2401, #2407): From and To layer dropdowns, the Simple /
//! Top-Middle-Bottom / Full Stack tiles and one size row per layer. The Lock
//! box is `mask::lock_row` (#2378).

use iced::widget::row;
use iced::{Element, Widget as _};
use signex_i18n::{UserText, ids};
use signex_types::pcb_stack::StackMode;
use signex_widgets::{button, text};

use super::rows::{Look, Section, dropdown, labelled_text, mm_field, pair, with_tip};
use super::{LayerChoice, LayerSizeRow, PcbPrimitiveMsg, PrimitiveCommon, PrimitiveField, SP, TS};
use crate::panels::{PanelMsg, form_look};

/// The From and To rows of a via's span; picking one keeps the other end.
pub(super) fn span_rows<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    layers: &'a [LayerChoice],
    ends: &'a (Option<LayerChoice>, Option<LayerChoice>),
    s: &mut Section<'a>,
) {
    let (top, bottom) = ends;
    let target = common.target.clone();
    let keep_bottom = bottom.as_ref().map(|c| c.layer);
    let from = dropdown(common, layers, top.clone(), move |choice| {
        PcbPrimitiveMsg::SpanPicked {
            target: target.clone(),
            top: choice.layer,
            bottom: keep_bottom.unwrap_or(choice.layer),
        }
    });
    let target = common.target.clone();
    let keep_top = top.as_ref().map(|c| c.layer);
    let to = dropdown(common, layers, bottom.clone(), move |choice| {
        PcbPrimitiveMsg::SpanPicked {
            target: target.clone(),
            top: keep_top.unwrap_or(choice.layer),
            bottom: choice.layer,
        }
    });
    s.push(
        common,
        ids::PCB_PRIM_SPAN_FROM,
        super::rows::labelled(look, ids::PCB_PRIM_SPAN_FROM, from),
    );
    s.push(
        common,
        ids::PCB_PRIM_SPAN_TO,
        super::rows::labelled(look, ids::PCB_PRIM_SPAN_TO, to),
    );
}

/// The three stack tiles; a press picks the mode for every selected item.
pub(super) fn mode_tiles<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    mode: Option<StackMode>,
) -> Element<'a, PanelMsg> {
    let options = [
        (StackMode::Simple, ids::PCB_PRIM_STACK_SIMPLE),
        (
            StackMode::TopMiddleBottom,
            ids::PCB_PRIM_STACK_TOP_MIDDLE_BOTTOM,
        ),
        (StackMode::Full, ids::PCB_PRIM_STACK_FULL),
    ];
    let mut tiles = row![].spacing(SP.xs);
    for (option, label) in options {
        let mut tile = button(text(look.t(label)).size(TS.body))
            .padding([SP.xs, SP.s])
            .style(form_look::option_tile(mode == Some(option)));
        if common.is_editable() && mode != Some(option) {
            tile = tile.on_press(PanelMsg::PcbPrimitive(PcbPrimitiveMsg::StackModePicked {
                target: common.target.clone(),
                mode: option,
            }));
        }
        tiles = tiles.push(tile);
    }
    with_tip(look, tiles.boxed(), ids::PCB_PRIM_STACK_TIP)
}

/// One row per layer size: a via's diameter, or a pad's x and y.
pub(super) fn size_rows<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    rows: &'a [LayerSizeRow],
    pad: bool,
    s: &mut Section<'a>,
) {
    for size in rows {
        let x = mm_field(common, PrimitiveField::LayerSize(size.layer), size.x, look);
        let value = if pad {
            let y = mm_field(common, PrimitiveField::LayerSizeY(size.layer), size.y, look);
            pair(x, y)
        } else {
            x
        };
        let label = UserText::new(size.label.clone());
        if common.shows(&look.ctx.i18n, &size.label) {
            s.push_row(labelled_text(look, label, value));
        }
    }
}
