//! Properties of the tracks, vias or pads selected on a board (#2355). One
//! panel per kind (`track`, `via`, `pad`) built from shared rows (`rows`)
//! and the shared Net Information section (`net_info`). A pure view over
//! facts the app copies from the board (`facts`); every edit is a
//! [`PcbPrimitiveMsg`] the app turns into one board command.
//!
//! The header (kind, count, search) is this module's own; the board panel
//! (#2353) and the component panel (#2354) carry the same shape, and the
//! three meet in `pcb_board_properties::view`, which picks the panel.

mod facts;
mod net_info;
mod pad;
mod rows;
mod track;
mod via;

use std::path::PathBuf;

use iced::widget::{column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_types::pcb::PadShape;
use signex_types::pcb_layer::CopperLayer;
use signex_widgets::{text, text_input};

pub use facts::{
    Editability, LayerChoice, NetChoice, NetInfo, PadFacts, PcbPrimitiveFacts, PrimitiveCommon,
    PrimitiveField, PrimitiveTarget, ShapeChoice, TrackFacts, ViaFacts, shared,
};

use super::{PanelContext, PanelMsg};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// An event of a primitive panel.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbPrimitiveMsg {
    FieldTyped {
        target: PrimitiveTarget,
        field: PrimitiveField,
        text: String,
    },
    /// Enter in a field: its draft is applied.
    FieldSubmitted {
        target: PrimitiveTarget,
        field: PrimitiveField,
    },
    LayerPicked {
        target: PrimitiveTarget,
        layer: CopperLayer,
    },
    NetPicked {
        target: PrimitiveTarget,
        net: u32,
    },
    ShapePicked {
        target: PrimitiveTarget,
        shape: PadShape,
    },
    /// The pick button: the next click on the board names the net.
    NetPickStarted { target: PrimitiveTarget },
    /// The net name link: select the net's copper.
    NetLinkPressed { board: PathBuf, net: u32 },
    /// The class link: select the copper of every net of the class.
    ClassLinkPressed { board: PathBuf, class: String },
    ShowMoreToggled,
    SearchTyped(String),
}

/// The panel for `facts`: header, then the kind's sections.
pub fn view<'a>(ctx: &'a PanelContext, facts: &'a PcbPrimitiveFacts) -> Element<'a, PanelMsg> {
    let body = match facts {
        PcbPrimitiveFacts::Track(f) => track::view(ctx, f),
        PcbPrimitiveFacts::Via(f) => via::view(ctx, f),
        PcbPrimitiveFacts::Pad(f) => pad::view(ctx, f),
    };
    let common = facts.common();
    let mut notes = column![].spacing(SP.xs);
    let muted = resolve(&ctx.tokens).ink3;
    let note = |id| text(t!(ctx.i18n, id)).size(TS.dense).color(muted);
    match common.editability {
        Editability::ReadOnly => notes = notes.push(note(ids::PCB_PRIM_READ_ONLY)),
        Editability::Locked => notes = notes.push(note(ids::PCB_PRIM_PAD_LOCKED)),
        Editability::Editable => {}
    }
    if common.picking_net {
        notes = notes.push(note(ids::PCB_PRIM_PICKING_NET));
    }
    if !common.drafts.is_empty() {
        notes = notes.push(note(ids::PCB_PRIM_ENTER_HINT));
    }
    column![
        header(ctx, facts),
        body,
        container(notes).padding([SP.s, SP.l]).width(Length::Fill),
    ]
    .width(Length::Fill)
    .boxed()
}

/// The kind, how many are selected, and the search field.
fn header<'a>(ctx: &'a PanelContext, facts: &'a PcbPrimitiveFacts) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let count = facts.common().target.items.len();
    let (title, summary) = match facts {
        PcbPrimitiveFacts::Track(_) => (
            ids::PCB_PRIM_TRACK_TITLE,
            t!(i18n, ids::PCB_PRIM_TRACK_COUNT, "count" => count),
        ),
        PcbPrimitiveFacts::Via(_) => (
            ids::PCB_PRIM_VIA_TITLE,
            t!(i18n, ids::PCB_PRIM_VIA_COUNT, "count" => count),
        ),
        PcbPrimitiveFacts::Pad(_) => (
            ids::PCB_PRIM_PAD_TITLE,
            t!(i18n, ids::PCB_PRIM_PAD_COUNT, "count" => count),
        ),
    };
    let ink = ti(ctx.tokens.text);
    let muted = resolve(&ctx.tokens).ink3;
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let title_row = row![
        text(t!(i18n, title)).size(TS.body).font(semibold).color(ink),
        text(summary)
            .size(TS.dense)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None)
            .width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    let search = text_input(t!(i18n, ids::PCB_PRIM_SEARCH), &facts.common().search)
        .on_input(|text| PanelMsg::PcbPrimitive(PcbPrimitiveMsg::SearchTyped(text)))
        .size(TS.body)
        .padding(SP.s)
        .style(super::form_look::text_input_style)
        .width(Length::Fill);
    container(column![title_row, search].spacing(SP.s))
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// A length or angle as a field shows it: up to four decimals, no trailing
/// zeros.
pub fn format_number(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// A typed length in mm or angle in degrees: a number, optionally followed
/// by its unit.
pub fn parse_number(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    let bare = trimmed
        .strip_suffix("mm")
        .or_else(|| trimmed.strip_suffix('°'))
        .unwrap_or(trimmed)
        .trim();
    bare.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_format_short_and_parse_with_or_without_a_unit() {
        assert_eq!(format_number(0.25), "0.25");
        assert_eq!(format_number(1.0), "1");
        assert_eq!(format_number(-0.00001), "0");
        assert_eq!(parse_number(" 0.3mm "), Some(0.3));
        assert_eq!(parse_number("45°"), Some(45.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("inf"), None);
    }
}
