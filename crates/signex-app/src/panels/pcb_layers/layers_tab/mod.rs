//! The Layers & Colors tab (#2363, mockup row 103), top to bottom: the search row
//! (with Used On and the more button), the name row while a new layer set
//! is named, the Active | Set row, All Layers, then the layers by section.

pub(in crate::panels::pcb_layers) mod active_row;
mod pairs;
mod sections;

pub(super) use sections::section_layers;

use iced::widget::tooltip::Position;
use iced::widget::{Column, Space, container, row, svg};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme, Widget};
use signex_i18n::{ids, t};
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{button, text, text_input};

use super::metrics::{ALL_LAYERS_ROW, EMPTY_PAD_X, EMPTY_PAD_Y, EYE, FIELD, ICON, ROW_GAP};
use super::rows::{eye, medium};
use super::{LayersPanelMsg, LayersPanelUi, PcbLayersPanel, SEARCH_PAD, SP, TS, row_button};
use crate::app::PcbViewMsg;
use crate::panels::form_look;
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{Radii, field, ghost_button, ti};

pub(super) fn view<'a>(
    panel: &'a PcbLayersPanel,
    ui: &'a LayersPanelUi,
    ctx: &'a PanelContext,
) -> Element<'a, PanelMsg> {
    let mut body = Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(search_row(panel, ui, ctx));
    if ui.naming {
        body = body.push(naming_row(panel, ui, ctx));
    }
    body = body
        .push(active_row::view(panel, ctx))
        .push(all_layers_row(panel, ctx));
    let sections = sections::all(panel, ui, ctx);
    if sections.is_empty() && !ui.search.trim().is_empty() {
        body = body.push(no_match(panel, ui, ctx));
    }
    sections
        .into_iter()
        .fold(body, |body, section| body.push(section))
        .push(Space::new().height(SP.l).boxed())
        .boxed()
}

fn panel_msg(msg: LayersPanelMsg) -> PanelMsg {
    PanelMsg::LayersPanel(msg)
}

/// The search field, Used On and the more button.
fn search_row<'a>(
    panel: &PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let (ink4, ink3, ink) = (r.ink4, r.ink3, ti(tokens.text));
    let icon = svg(crate::icons::icon_chrome_search(ctx.theme_id))
        .width(ICON)
        .height(ICON)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink4),
            ..Default::default()
        });
    let accent = ti(tokens.accent);
    let input = text_input(t!(i18n, ids::PCB_LAYERS_SEARCH), ui.search.clone())
        .on_input(|query| panel_msg(LayersPanelMsg::SearchChanged(query)))
        .size(TS.body)
        .padding(0)
        .style(move |_: &Theme, _| iced::widget::text_input::Style {
            background: Background::Color(iced::Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: iced::Color { a: 0.4, ..accent },
        })
        .width(Length::Fill);
    let search = container(
        row![icon, input]
            .spacing(ROW_GAP)
            .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .center_y(FIELD)
    .style(field(tokens, false));

    let used_on = panel.used.is_some();
    let style = ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: hover_overlay(tokens),
        ink: ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    let used = hint_tooltip(
        chip(
            t!(i18n, ids::PCB_LAYERS_USED_ON),
            None,
            used_on,
            style,
            ChipMetrics::from_text_size(TS.body),
            i18n,
            Some(PanelMsg::PcbLayers(PcbViewMsg::UsedLayersOnlySet(!used_on))),
        ),
        t!(i18n, ids::PCB_LAYERS_USED_ON_TIP),
        tokens,
        Position::Bottom,
    );

    let open = ui.more_menu_open;
    let (open_fill, line, wash) = (r.accent_soft, ti(tokens.border), hover_overlay(tokens));
    let more_icon = svg(crate::icons::icon_chrome_more(ctx.theme_id))
        .width(ICON)
        .height(ICON)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink3),
            ..Default::default()
        });
    let more = button(container(more_icon).center_x(FIELD).center_y(FIELD))
        .padding(0)
        .style(move |_: &Theme, status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: if open {
                    Some(Background::Color(open_fill))
                } else {
                    hovered.then_some(Background::Color(wash))
                },
                border: Border {
                    width: 1.0,
                    color: line,
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(panel_msg(LayersPanelMsg::MoreMenuToggled));
    let more = hint_tooltip(
        more.boxed(),
        t!(i18n, ids::PCB_LAYERS_MORE_TIP),
        tokens,
        Position::Bottom,
    );
    container(
        row![search, used, more]
            .spacing(SP.s)
            .align_y(Alignment::Center),
    )
    .padding(SEARCH_PAD)
    .width(Length::Fill)
    .boxed()
}

/// Save current as set...: the name field, Save (Enter too) and a close
/// button that drops the name.
fn naming_row<'a>(
    panel: &PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let save = (!ui.set_name.trim().is_empty()).then_some(panel_msg(LayersPanelMsg::SetSaved));
    let name = text_input(
        t!(i18n, ids::PCB_LAYER_SET_NAME_PLACEHOLDER),
        ui.set_name.clone(),
    )
    .id(LayersPanelUi::SET_NAME_INPUT)
    .on_input(|name| panel_msg(LayersPanelMsg::SetNameChanged(name)))
    .on_submit_maybe(save.clone())
    .size(TS.body)
    .padding(SP.s)
    .style(form_look::text_input_style)
    .width(Length::Fill);
    let save = button(text(t!(i18n, ids::PCB_LAYER_SET_SAVE_BUTTON)).size(TS.body))
        .padding([SP.xs, SP.m])
        .style(form_look::ghost_button)
        .on_press_maybe(save);
    let ink3 = resolve(&ctx.tokens).ink3;
    let close = svg(crate::icons::icon_close(ctx.theme_id))
        .width(ICON)
        .height(ICON)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink3),
            ..Default::default()
        });
    let cancel = button(container(close).center_x(EYE).center_y(EYE))
        .padding(0)
        .style(row_button(&ctx.tokens))
        .on_press(panel_msg(LayersPanelMsg::SetNamingCancelled));
    container(
        row![name, save, cancel]
            .spacing(SP.s)
            .align_y(Alignment::Center),
    )
    .padding(Padding {
        top: 0.0,
        bottom: SP.s,
        left: SP.m,
        right: SP.m,
    })
    .width(Length::Fill)
    .boxed()
}

/// All Layers: an eye that shows every layer while any is hidden, else
/// hides them all.
fn all_layers_row<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let toggled = if panel.has_hidden() {
        PcbViewMsg::AllLayersShown
    } else {
        PcbViewMsg::LayerKindVisibilitySet {
            kind: signex_renderer::pcb::LayerKind::All,
            visible: false,
        }
    };
    let ink3 = resolve(&ctx.tokens).ink3;
    container(
        row![
            eye(
                panel,
                panel.has_hidden(),
                Some(PanelMsg::PcbLayers(toggled)),
                ctx
            ),
            text(t!(panel.i18n, ids::PCB_LAYERS_ALL_LAYERS))
                .size(TS.body)
                .color(ink3)
                .width(Length::Fill),
        ]
        .spacing(ROW_GAP)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .center_y(ALL_LAYERS_ROW)
    .boxed()
}

/// No row passes the search: the query, and Clear.
fn no_match<'a>(
    panel: &PcbLayersPanel,
    ui: &LayersPanelUi,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let accent_text = resolve(&ctx.tokens).accent_text;
    let line = text(t!(i18n, ids::PCB_LAYERS_NO_MATCH, "query" => ui.search.trim()))
        .size(TS.body)
        .font(medium())
        .color(ti(ctx.tokens.text));
    let clear = button(
        text(t!(i18n, ids::PCB_LAYERS_CLEAR_SEARCH))
            .size(TS.caption)
            .color(accent_text),
    )
    .padding([SP.s, SP.m])
    .style(ghost_button(&ctx.tokens))
    .on_press(panel_msg(LayersPanelMsg::SearchChanged(String::new())));
    container(
        Column::<Element<'a, PanelMsg>>::new()
            .spacing(SP.xs)
            .align_x(iced::alignment::Horizontal::Center)
            .push(line.boxed())
            .push(clear.boxed()),
    )
    .padding([EMPTY_PAD_Y, EMPTY_PAD_X])
    .center_x(Length::Fill)
    .boxed()
}
