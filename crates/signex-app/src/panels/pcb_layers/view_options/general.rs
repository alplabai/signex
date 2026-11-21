//! The Configuration row above the sections, and General Settings: 3D,
//! Single Layer and Show Grid.

use iced::widget::tooltip::Position;
use iced::widget::{Row, container, mouse_area, row};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme, Widget, mouse};
use signex_i18n::{Localized, MessageId, ids, t};
use signex_widgets::chevron::chevron;
use signex_widgets::{ToggleSwitchStyle, button, text, toggle_switch};

use super::super::rows::not_yet;
use super::super::{LayersPanelMsg, PcbLayersPanel, SP, TS};
use super::LINE_H;
use crate::app::PcbViewMsg;
use crate::panels::form_look::{self, segment_cell};
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::tokens::{accent_fill, on_accent, resolve};
use crate::styles::{ChromeMetrics, Radii, chrome_px, ti};

/// The caption column of the Configuration row.
const PRESET_CAPTION_W: f32 = chrome_px(80.0 * 11.0 / 12.0);
/// The caption column of a General Settings row.
const CAPTION_W: f32 = chrome_px(92.0 * 11.0 / 12.0);
/// A cell of a small segmented switch.
const CELL_H: f32 = chrome_px(20.0 * 11.0 / 12.0);

/// Configuration: a caption, then the preset field reading Default. Greyed
/// until presets exist (#2480): it opens nothing.
pub(super) fn configuration<'a>(
    panel: &PcbLayersPanel,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let tokens = resolve(&ctx.tokens);
    let (ink4, field, line) = (tokens.ink4, tokens.field, ti(ctx.tokens.border));
    let value = row![
        text(t!(i18n, ids::PCB_VIEW_PRESET_DEFAULT))
            .size(TS.body)
            .color(ink4)
            .width(Length::Fill),
        chevron(true, TS.caption, ink4),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let dropdown = container(value)
        .width(Length::Fill)
        .center_y(ChromeMetrics::DEFAULT.row)
        .padding([0.0, SP.s])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(field)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed();
    let caption = text(t!(i18n, ids::PCB_VIEW_PRESET))
        .size(TS.caption)
        .color(ink4)
        .width(PRESET_CAPTION_W);
    container(
        row![caption, not_yet(dropdown, panel, ctx)]
            .spacing(SP.s)
            .align_y(Alignment::Center),
    )
    .padding(Padding {
        top: SP.xs,
        right: SP.m,
        bottom: SP.s,
        left: SP.m,
    })
    .width(Length::Fill)
    .boxed()
}

/// General Settings: 3D, Single Layer and Show Grid.
pub(super) fn rows<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    let toggle_3d = Some(PanelMsg::LayersPanel(LayersPanelMsg::Board3dToggled));
    let three_d = segmented(vec![
        cell(
            panel,
            ctx,
            ids::PCB_VIEW_OFF,
            !panel.board_3d,
            toggle_3d.clone().filter(|_| panel.board_3d),
        ),
        cell(
            panel,
            ctx,
            ids::PCB_VIEW_ON,
            panel.board_3d,
            toggle_3d.filter(|_| !panel.board_3d),
        ),
    ]);
    let toggle_single = Some(PanelMsg::PcbLayers(PcbViewMsg::SingleLayerModeToggled));
    let single = segmented(vec![
        cell(
            panel,
            ctx,
            ids::PCB_VIEW_OFF,
            !panel.single_layer,
            toggle_single.clone().filter(|_| panel.single_layer),
        ),
        cell(
            panel,
            ctx,
            ids::PCB_VIEW_ON,
            panel.single_layer,
            toggle_single.filter(|_| !panel.single_layer),
        ),
        cell(panel, ctx, ids::PCB_VIEW_MONO, false, None),
    ]);
    let single_caption = hint_tooltip(
        caption(ctx, t!(i18n, ids::PCB_VIEW_SINGLE_LAYER_SHORT)),
        t!(i18n, ids::PCB_VIEW_SINGLE_LAYER),
        &ctx.tokens,
        Position::Top,
    );
    let tokens = resolve(&ctx.tokens);
    let grid = toggle_switch(
        panel.grid_visible,
        ToggleSwitchStyle::new(
            accent_fill(&ctx.tokens),
            tokens.line_strong,
            on_accent(&ctx.tokens),
        ),
        Some(PanelMsg::ToggleGrid),
    );
    // The caption toggles the grid too, as the checkbox label did.
    let grid_caption = mouse_area(caption(ctx, t!(i18n, ids::PCB_VIEW_SHOW_GRID)))
        .on_press(PanelMsg::ToggleGrid)
        .interaction(mouse::Interaction::Pointer)
        .boxed();
    vec![
        line(caption(ctx, t!(i18n, ids::PCB_VIEW_3D)), three_d),
        line(single_caption, single),
        line(grid_caption, grid),
    ]
}

/// The muted caption of a General Settings row.
fn caption<'a>(ctx: &PanelContext, label: Localized) -> Element<'a, PanelMsg> {
    text(label)
        .size(TS.body)
        .color(resolve(&ctx.tokens).ink3)
        .boxed()
}

/// `caption` in its fixed column, then `control` in the rest.
fn line<'a>(
    caption: Element<'a, PanelMsg>,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    container(
        row![
            container(caption).width(CAPTION_W),
            container(control).width(Length::Fill),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding(Padding {
        top: 0.0,
        right: SP.m,
        bottom: 0.0,
        left: SP.m + SP.s,
    })
    .center_y(LINE_H)
    .width(Length::Fill)
    .boxed()
}

/// One cell of a small segmented switch: pressed sends `msg`; a cell
/// without one is greyed with the "not yet" tooltip.
fn cell<'a>(
    panel: &PcbLayersPanel,
    ctx: &PanelContext,
    label: MessageId,
    on: bool,
    msg: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let greyed = msg.is_none() && !on;
    let label = text(t!(panel.i18n, label))
        .size(TS.caption)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center);
    let cell = button(container(label).center_y(Length::Fill))
        .padding([0.0, SP.s])
        .width(Length::Fill)
        .height(CELL_H)
        .on_press_maybe(msg)
        .style(
            move |theme: &iced::Theme, status: iced::widget::button::Status| {
                let hovered = matches!(status, iced::widget::button::Status::Hovered);
                let mut style = segment_cell(theme, on, hovered);
                if greyed {
                    style.text_color = form_look::FormLook::of(theme).ink4;
                }
                style
            },
        )
        .boxed();
    if greyed {
        not_yet(cell, panel, ctx)
    } else {
        cell
    }
}

/// The frame around the cells of a segmented switch, as wide as its column.
fn segmented<'a>(cells: Vec<Element<'a, PanelMsg>>) -> Element<'a, PanelMsg> {
    container(Row::with_children(cells).spacing(SP.xs).width(Length::Fill))
        .padding(SP.xs)
        .width(Length::Fill)
        .style(form_look::segmented_frame)
        .boxed()
}
