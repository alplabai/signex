//! The drawing panel (#2424, #2523 m102): a board line, arc, circle,
//! rectangle or text frame. The illustration, then the layer, line width
//! and (a line's) ends of any drawing; Location: the corners of a line,
//! rectangle or text frame, the centre and radius of an arc or circle; a
//! text frame's text and text height.

use iced::widget::container;
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids};
use signex_types::pcb_graphic::BoardGraphicKind;

use super::drawing::{self, keys};
use super::linked::{self, linked_row, prefixed, two_fields, unit};
use super::rows::{Look, PAD_X, Section, any_layer_value, field, stack};
use super::{GraphicFacts, PrimitiveField as F, SP, TS};
use crate::panels::{PanelContext, PanelMsg};
use signex_widgets::property_illustration::FieldId;
use signex_widgets::text;

/// The header title for drawings of `kind`; mixed kinds read "Drawings".
pub(super) fn title(kind: Option<BoardGraphicKind>) -> MessageId {
    kind.map_or(ids::PCB_PRIM_GRAPHIC_TITLE, |kind| {
        crate::app::pcb_tools::BoardPlaceTool::for_graphic(kind).name()
    })
}

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a GraphicFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let drawn = drawing::graphic::illustration(&ctx.i18n, f)
        .map(|drawn| linked::illustration(look, &f.common, drawn));
    stack(vec![
        drawn,
        properties(look, f),
        geometry(look, f),
        text_section(look, f),
    ])
}

fn properties<'a>(look: Look<'a>, f: &'a GraphicFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES).collapsible(
        "pcb_prim.graphic.properties",
        false,
        None,
    );
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        linked_row(
            look,
            common,
            keys::LAYER,
            ids::PCB_PRIM_LAYER,
            any_layer_value(look, common, &f.layers, f.layer.as_ref()),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_WIDTH,
        linked_row(
            look,
            common,
            keys::WIDTH,
            ids::PCB_PRIM_WIDTH,
            linked::mm(look, common, F::Width, f.width),
        ),
    );
    // A line's ends; the board stores no cap for a drawing, so they are
    // round and Square and Flat wait greyed.
    if f.kind == Some(BoardGraphicKind::Line) {
        s.push(
            common,
            ids::PCB_PRIM_ENDS,
            linked_row(
                look,
                common,
                keys::ENDS,
                ids::PCB_PRIM_ENDS,
                linked::ends(look, false),
            ),
        );
    }
    s.finish()
}

/// The points a drawing of the shared kind is edited by; nothing for a
/// mixed selection or the drawing about to be placed, whose points are the
/// pointer's.
fn geometry<'a>(look: Look<'a>, f: &'a GraphicFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let kind = f.kind.filter(|_| !common.target.is_placement())?;
    let mut s = Section::new(look, ids::PCB_PRIM_LOCATION).collapsible(
        "pcb_prim.graphic.location",
        false,
        None,
    );
    let xy = |key: FieldId, label: MessageId, fx: F, fy: F, at: (Option<f64>, Option<f64>)| {
        linked_row(
            look,
            common,
            key,
            label,
            two_fields(
                prefixed(look, common, fx, at.0, ids::PCB_PRIM_X),
                prefixed(look, common, fy, at.1, ids::PCB_PRIM_Y),
                unit(look, ids::PCB_PRIM_UNIT_MM),
            ),
        )
    };
    match kind {
        BoardGraphicKind::Arc | BoardGraphicKind::Circle => {
            s.push(
                common,
                ids::PCB_PRIM_CENTRE,
                xy(
                    keys::CENTRE,
                    ids::PCB_PRIM_CENTRE,
                    F::CenterX,
                    F::CenterY,
                    f.center,
                ),
            );
            s.push(
                common,
                ids::PCB_PRIM_GRAPHIC_RADIUS,
                linked_row(
                    look,
                    common,
                    keys::RADIUS,
                    ids::PCB_PRIM_GRAPHIC_RADIUS,
                    linked::mm(look, common, F::Radius, f.radius),
                ),
            );
        }
        BoardGraphicKind::Line | BoardGraphicKind::Rectangle | BoardGraphicKind::TextFrame => {
            s.push(
                common,
                ids::PCB_PRIM_POINT_START,
                xy(
                    keys::START,
                    ids::PCB_PRIM_POINT_START,
                    F::StartX,
                    F::StartY,
                    f.start,
                ),
            );
            s.push(
                common,
                ids::PCB_PRIM_POINT_END,
                xy(keys::END, ids::PCB_PRIM_POINT_END, F::EndX, F::EndY, f.end),
            );
        }
    }
    s.finish()
}

/// A text frame's text (the whole row wide), its hint, and its text height.
fn text_section<'a>(look: Look<'a>, f: &'a GraphicFacts) -> Option<Element<'a, PanelMsg>> {
    if f.kind != Some(BoardGraphicKind::TextFrame) {
        return None;
    }
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_GRAPHIC_TEXT).collapsible(
        "pcb_prim.graphic.text",
        false,
        None,
    );
    let hot = linked::is_hot(common, &keys::TEXT);
    let wide = container(linked::ring(
        look,
        hot,
        field(common, F::Text, f.text.clone(), None, look),
    ))
    .padding([SP.xs, PAD_X])
    .width(Length::Fill)
    .boxed();
    s.push(
        common,
        ids::PCB_PRIM_GRAPHIC_TEXT,
        linked::linked(look, common, keys::TEXT, wide),
    );
    s.push(
        common,
        ids::PCB_PRIM_GRAPHIC_TEXT,
        container(
            text(look.t(ids::PCB_PRIM_GRAPHIC_TEXT_HINT))
                .size(TS.caption)
                .color(crate::styles::tokens::resolve(look.tokens()).ink4),
        )
        .padding([0.0, PAD_X])
        .width(Length::Fill)
        .boxed(),
    );
    s.push(
        common,
        ids::PCB_PRIM_GRAPHIC_TEXT_HEIGHT,
        linked_row(
            look,
            common,
            keys::TEXT_HEIGHT,
            ids::PCB_PRIM_GRAPHIC_TEXT_HEIGHT,
            linked::mm(look, common, F::TextHeight, f.text_height),
        ),
    );
    s.finish()
}
