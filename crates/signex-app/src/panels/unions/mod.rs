//! The Unions dock panel (mockup row 158): a filter field, the This
//! sheet / Project scope switch and the tag chips over the list of unions
//! (`rows`), and the picked union's members and actions at the foot
//! (`foot`). A pure view over `UnionsPanelView`, which `update` builds
//! (`app/unions/state.rs`).

mod foot;
mod rows;

use std::sync::OnceLock;

use iced::widget::{Column, Row, column, container, row, scrollable, svg};
use iced::{Alignment, Background, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{SegmentedStyle, button, segmented, text, text_input};

use super::{PanelContext, PanelMsg, empty_state_card};
use crate::app::unions::{UnionScope, UnionsMsg};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn um(msg: UnionsMsg) -> PanelMsg {
    PanelMsg::Unions(msg)
}

/// Filter-chip colours of the Graphite tokens.
pub(crate) fn chip_style(tokens: &ThemeTokens) -> ChipStyle {
    let r = resolve(tokens);
    ChipStyle {
        fill: r.field,
        edge: r.line_strong,
        hover: r.sunken,
        ink: ti(tokens.text_secondary),
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

/// Segmented-switch colours of the Graphite tokens.
pub(crate) fn segmented_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = resolve(tokens);
    SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        ti(tokens.text_secondary),
        ti(tokens.text),
        TS.body,
    )
}

const SEARCH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><circle cx="6" cy="6" r="4" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M9 9l3.5 3.5" stroke="currentColor" stroke-width="1.3"/></svg>"#;
const PLUS: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M7 2v10M2 7h10" stroke="currentColor" stroke-width="1.4"/></svg>"#;

fn handle(cell: &'static OnceLock<svg::Handle>, bytes: &'static [u8]) -> svg::Handle {
    cell.get_or_init(|| svg::Handle::from_memory(bytes)).clone()
}

/// A tinted 14 px icon.
pub(super) fn icon<'a>(handle: svg::Handle, ink: Color) -> Element<'a, PanelMsg> {
    let side = TS.body + SP.xs;
    svg(handle)
        .width(side)
        .height(side)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// The panel.
pub fn view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let v = &ctx.unions;
    let tokens = &ctx.tokens;
    if !v.has_sheet {
        return empty_state_card(
            tokens,
            t!(ctx.i18n, ids::UNIONS_PANEL_EMPTY_TITLE),
            t!(ctx.i18n, ids::UNIONS_PANEL_EMPTY_BODY),
            None,
        );
    }
    let header = container(head(ctx)).padding(SP.m).width(Length::Fill);
    let mut col: Column<Element<'_, PanelMsg>> = Column::new().width(Length::Fill);
    col = col.push(header.boxed());
    col = col.push(rule(tokens));
    col = col.push(
        scrollable(rows::list(ctx))
            .height(Length::Fill)
            .width(Length::Fill)
            .boxed(),
    );
    if let Some(foot) = foot::view(ctx) {
        col = col.push(rule(tokens));
        col = col.push(foot);
    }
    col.height(Length::Fill).boxed()
}

/// A 1 px line-soft rule across the panel.
pub(super) fn rule<'a>(tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let colour = resolve(tokens).line_soft;
    container(iced::widget::Space::new().height(1.0))
        .width(Length::Fill)
        .height(1.0)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            ..container::Style::default()
        })
        .boxed()
}

/// Filter field and Create from selection, the scope switch, the tag chips.
fn head(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    static SEARCH_ICON: OnceLock<svg::Handle> = OnceLock::new();
    static PLUS_ICON: OnceLock<svg::Handle> = OnceLock::new();
    let v = &ctx.unions;
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let filter = row![
        icon(handle(&SEARCH_ICON, SEARCH), r.ink3),
        text_input(t!(ctx.i18n, ids::UNIONS_PANEL_FILTER), &v.filter)
            .on_input(|s| um(UnionsMsg::FilterEdited(s)))
            .size(TS.body)
            .width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let create = signex_widgets::tooltip(
        button(icon(handle(&PLUS_ICON, PLUS), ti(tokens.text_secondary)))
            .padding(SP.xs)
            .style(crate::styles::ghost_button(tokens))
            .on_press_maybe(v.can_create.then_some(um(UnionsMsg::CreateRequested))),
        container(text(t!(ctx.i18n, ids::UNIONS_PANEL_CREATE)).size(TS.caption))
            .padding([SP.xs, SP.s])
            .style(crate::styles::context_menu(tokens)),
        iced::widget::tooltip::Position::Bottom,
    );
    let top = row![filter, create]
        .spacing(SP.s)
        .align_y(Alignment::Center);
    let scope = segmented(
        [
            (
                UnionScope::ThisSheet,
                t!(ctx.i18n, ids::UNIONS_PANEL_THIS_SHEET).into(),
            ),
            (
                UnionScope::Project,
                t!(ctx.i18n, ids::UNIONS_PANEL_PROJECT).into(),
            ),
        ],
        v.scope,
        segmented_style(tokens).fill_width(true),
        |scope| um(UnionsMsg::ScopePicked(scope)),
    );
    let metrics = ChipMetrics::from_text_size(TS.body);
    let mut tags: Row<Element<'_, PanelMsg>> =
        Row::new().spacing(SP.xs).align_y(Alignment::Center).push(
            text(t!(ctx.i18n, ids::UNIONS_PANEL_TAGS))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    for (tag, on) in &v.tags {
        tags = tags.push(chip(
            UserText::new(format!("#{tag}")),
            None,
            *on,
            chip_style(tokens),
            metrics,
            &ctx.i18n,
            Some(um(UnionsMsg::TagChipToggled(tag.clone()))),
        ));
    }
    column![top, scope, tags.wrap().vertical_spacing(SP.xs)]
        .spacing(SP.s)
        .boxed()
}
