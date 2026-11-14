//! The Component view's header: the title with the selection summary beside
//! it, the row search and the General / Parameters tabs.
//!
//! The board's "nothing selected" Properties (#2353) draws the same header
//! shape; this one is self-contained, and the two converge on one shared
//! header builder once both have landed.

use iced::widget::{Column, Row, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{text, text_input};

use super::{PcbComponentFacts, PcbComponentMsg, Shared};
use crate::panels::{PanelContext, PanelMsg, form_look, props_tab_btn};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The header block.
pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    facts: &'a PcbComponentFacts,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let (ink, muted) = (ti(tokens.text), resolve(tokens).ink3);
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    // One component is named by its designator; several by their count.
    let summary: signex_widgets::Label<'a> = match (&facts.designator, facts.footprints.len()) {
        (Shared::Same(designator), 1) => UserText::new(designator.clone()).into(),
        (_, count) => t!(i18n, ids::PCB_COMPONENT_COUNT, "count" => count).into(),
    };
    let title = Row::<Element<'a, PanelMsg>>::new()
        .push_boxed(
            text(t!(i18n, ids::PCB_COMPONENT_TITLE))
                .size(TS.body)
                .font(semibold)
                .color(ink),
        )
        .push_boxed(
            text(summary)
                .size(TS.dense)
                .color(muted)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .width(Length::Fill),
        )
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);
    let search = text_input(t!(i18n, ids::PCB_COMPONENT_SEARCH), facts.search.clone())
        .on_input(|query| PanelMsg::PcbComponent(PcbComponentMsg::SearchChanged(query)))
        .size(TS.body)
        .padding(SP.s)
        .style(form_look::text_input_style)
        .width(Length::Fill);
    let tab = ctx.properties_tab;
    let inactive = ti(tokens.text_secondary);
    let tabs = Row::new()
        .push(props_tab_btn(
            t!(i18n, ids::PANELS_PROPERTIES_TAB_GENERAL),
            tab != 1,
            PanelMsg::PropertiesTab(0),
            ink,
            inactive,
        ))
        .push(props_tab_btn(
            t!(i18n, ids::PANELS_PROPERTIES_TAB_PARAMETERS),
            tab == 1,
            PanelMsg::PropertiesTab(1),
            ink,
            inactive,
        ))
        .spacing(SP.xs);
    container(
        Column::<Element<'a, PanelMsg>>::new()
            .push_boxed(title)
            .push_boxed(search)
            .push_boxed(tabs)
            .spacing(SP.m),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill)
    .boxed()
}
