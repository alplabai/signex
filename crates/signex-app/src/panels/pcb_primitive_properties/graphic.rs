//! The drawing panel (#2424): a board line, arc, circle, rectangle or text
//! frame. The layer and line width of any drawing; the corners of a line,
//! rectangle or text frame; the centre and radius of an arc or circle; a
//! text frame's text and text height.

use iced::widget::{container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids};
use signex_types::pcb_graphic::BoardGraphicKind;

use super::rows::{Look, Section, chip, dropdown, field, labelled, mm_field, pair, stack};
use super::{GraphicFacts, PcbPrimitiveMsg, PrimitiveField as F, SP, TS};
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;
use signex_widgets::text;

/// The header title for drawings of `kind`; mixed kinds read "Drawings".
pub(super) fn title(kind: Option<BoardGraphicKind>) -> MessageId {
    kind.map_or(ids::PCB_PRIM_GRAPHIC_TITLE, |kind| {
        crate::app::pcb_tools::BoardPlaceTool::for_graphic(kind).name()
    })
}

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a GraphicFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    stack(vec![
        properties(look, f),
        geometry(look, f),
        text_section(look, f),
    ])
}

fn properties<'a>(look: Look<'a>, f: &'a GraphicFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES);
    let target = common.target.clone();
    let layers = dropdown(common, &f.layers, f.layer.clone(), move |choice| {
        PcbPrimitiveMsg::AreaLayerPicked {
            target: target.clone(),
            layer: choice.layer,
        }
    });
    let swatch = f.layer.as_ref().map_or_else(
        || {
            container(iced::widget::Space::new())
                .width(Length::Fixed(TS.body))
                .boxed()
        },
        |l| chip(layer_style::layer_colour(l.layer, look.tokens())),
    );
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        labelled(
            look,
            ids::PCB_PRIM_LAYER,
            row![swatch, container(layers).width(Length::Fill)]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed(),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_WIDTH,
        labelled(
            look,
            ids::PCB_PRIM_WIDTH,
            mm_field(common, F::Width, f.width, look),
        ),
    );
    s.finish()
}

/// The points a drawing of the shared kind is edited by; nothing for a
/// mixed selection or the drawing about to be placed, whose points are the
/// pointer's.
fn geometry<'a>(look: Look<'a>, f: &'a GraphicFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let kind = f.kind.filter(|_| !common.target.is_placement())?;
    let mut s = Section::new(look, ids::PCB_PRIM_LOCATION);
    let xy = |label: MessageId, fx: F, fy: F, at: (Option<f64>, Option<f64>)| {
        labelled(
            look,
            label,
            pair(
                mm_field(common, fx, at.0, look),
                mm_field(common, fy, at.1, look),
            ),
        )
    };
    match kind {
        BoardGraphicKind::Arc | BoardGraphicKind::Circle => {
            s.push(
                common,
                ids::PCB_PRIM_GRAPHIC_CENTER,
                xy(
                    ids::PCB_PRIM_GRAPHIC_CENTER,
                    F::CenterX,
                    F::CenterY,
                    f.center,
                ),
            );
            s.push(
                common,
                ids::PCB_PRIM_GRAPHIC_RADIUS,
                labelled(
                    look,
                    ids::PCB_PRIM_GRAPHIC_RADIUS,
                    mm_field(common, F::Radius, f.radius, look),
                ),
            );
        }
        BoardGraphicKind::Line | BoardGraphicKind::Rectangle | BoardGraphicKind::TextFrame => {
            s.push(
                common,
                ids::PCB_PRIM_START,
                xy(ids::PCB_PRIM_START, F::StartX, F::StartY, f.start),
            );
            s.push(
                common,
                ids::PCB_PRIM_END,
                xy(ids::PCB_PRIM_END, F::EndX, F::EndY, f.end),
            );
        }
    }
    s.finish()
}

/// A text frame's text and text height.
fn text_section<'a>(look: Look<'a>, f: &'a GraphicFacts) -> Option<Element<'a, PanelMsg>> {
    if f.kind != Some(BoardGraphicKind::TextFrame) {
        return None;
    }
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_GRAPHIC_TEXT);
    s.push(
        common,
        ids::PCB_PRIM_GRAPHIC_TEXT,
        labelled(
            look,
            ids::PCB_PRIM_GRAPHIC_TEXT,
            field(common, F::Text, f.text.clone(), None, look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_GRAPHIC_TEXT_HEIGHT,
        labelled(
            look,
            ids::PCB_PRIM_GRAPHIC_TEXT_HEIGHT,
            mm_field(common, F::TextHeight, f.text_height, look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_GRAPHIC_TEXT,
        container(
            text(look.t(ids::PCB_PRIM_GRAPHIC_TEXT_HINT))
                .size(TS.dense)
                .color(look.muted),
        )
        .padding([SP.xs, super::rows::PAD_X])
        .width(Length::Fill)
        .boxed(),
    );
    s.finish()
}
