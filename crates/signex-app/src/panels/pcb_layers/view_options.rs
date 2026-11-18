//! The View Options tab (#2363): 3D, single-layer mode and the grid, then
//! the kinds of object, each with an eye, Draft and a transparency. A
//! control the board view cannot honour yet is greyed with a tooltip.

use iced::widget::{Column, Row, container, row, slider};
use iced::{Alignment, Element, Length, Widget};
use signex_i18n::{Localized, MessageId, ids, t};
use signex_renderer::pcb::{BoardObject, DIMMED_ALPHA};
use signex_widgets::{CheckboxLabel as _, button, checkbox, text};

use super::rows::{chip, eye, not_yet, section};
use super::{LayersPanelMsg, PcbLayersPanel, SP, TS, toggle_button};
use crate::app::PcbViewMsg;
use crate::panels::form_look::{self, segment_cell};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::tokens::resolve;

pub(super) fn view<'a>(panel: &'a PcbLayersPanel, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let sections = [
        section(
            ctx,
            "pcb_view_general",
            t!(i18n, ids::PCB_VIEW_GENERAL),
            general_rows(panel, ctx),
        ),
        section(
            ctx,
            "pcb_view_objects",
            t!(i18n, ids::PCB_VIEW_OBJECTS),
            object_rows(panel, ctx),
        ),
        section(
            ctx,
            "pcb_view_mask_dim",
            t!(i18n, ids::PCB_VIEW_MASK_DIM),
            mask_dim_rows(panel, ctx),
        ),
        section(
            ctx,
            "pcb_view_additional",
            t!(i18n, ids::PCB_VIEW_ADDITIONAL),
            vec![additional_chips(panel, ctx)],
        ),
    ];
    sections
        .into_iter()
        .flatten()
        .fold(
            Column::<Element<'a, PanelMsg>>::new()
                .spacing(SP.xs)
                .width(Length::Fill),
            |col, section| col.push(section),
        )
        .boxed()
}

/// A caption on the left and `control` on the right, padded as a row.
fn line<'a>(caption: Localized, control: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    container(
        row![text(caption).size(TS.body).width(Length::Fill), control]
            .spacing(SP.s)
            .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// One cell of a segmented switch: pressed sends `msg`; a cell without one
/// is greyed with the "not yet" tooltip.
fn cell<'a>(
    panel: &PcbLayersPanel,
    ctx: &PanelContext,
    label: MessageId,
    on: bool,
    msg: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let greyed = msg.is_none() && !on;
    let cell = button(text(t!(panel.i18n, label)).size(TS.body))
        .padding([SP.xs, SP.m])
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

fn segmented<'a>(cells: Vec<Element<'a, PanelMsg>>) -> Element<'a, PanelMsg> {
    container(Row::with_children(cells).spacing(SP.xs))
        .padding(SP.xs)
        .style(form_look::segmented_frame)
        .boxed()
}

/// General Settings: the configuration preset, 3D, single-layer mode and
/// the grid.
fn general_rows<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    let preset = not_yet(
        button(text(t!(i18n, ids::PCB_VIEW_PRESET_DEFAULT)).size(TS.body))
            .padding([SP.xs, SP.m])
            .style(toggle_button(&ctx.tokens, false))
            .boxed(),
        panel,
        ctx,
    );
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
    let grid = row![
        checkbox(panel.grid_visible)
            .size(TS.body)
            .labelled(t!(i18n, ids::PCB_VIEW_SHOW_GRID))
            .text_size(TS.body)
            .style(form_look::checkbox_style)
            .on_toggle(|_| PanelMsg::ToggleGrid),
        not_yet(
            chip(
                panel,
                crate::pcb_canvas::layer_style::grid_colour(),
                false,
                None,
                ctx
            ),
            panel,
            ctx,
        ),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    vec![
        line(t!(i18n, ids::PCB_VIEW_PRESET), preset),
        line(t!(i18n, ids::PCB_VIEW_3D), three_d),
        line(t!(i18n, ids::PCB_VIEW_SINGLE_LAYER), single),
        container(grid).padding([SP.xs, SP.m]).boxed(),
    ]
}

/// Whether every kind the view can hide is shown (`Some(true)`), every one
/// hidden (`Some(false)`), or they differ (`None`: Mixed).
fn all_objects_shown(panel: &PcbLayersPanel) -> Option<bool> {
    let mut kinds = BoardObject::ALL.into_iter().filter(|k| k.can_hide());
    let first = panel.shows_object(kinds.next()?);
    kinds
        .all(|kind| panel.shows_object(kind) == first)
        .then_some(first)
}

/// Object Visibility: All Objects, then one row per kind.
fn object_rows<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    let muted = resolve(&ctx.tokens).ink3;
    let all = all_objects_shown(panel);
    let all_eye = eye(
        panel,
        all == Some(false),
        Some(PanelMsg::PcbLayers(PcbViewMsg::ObjectVisibilitySet {
            kind: None,
            visible: all != Some(true),
        })),
        ctx,
    );
    let mut all_row = row![
        all_eye,
        text(t!(i18n, ids::PCB_VIEW_OBJECT, "kind" => "all"))
            .size(TS.body)
            .width(Length::Fill),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    if all.is_none() {
        all_row = all_row.push(
            text(t!(i18n, ids::PCB_VIEW_MIXED))
                .size(TS.caption)
                .color(muted),
        );
    }
    let header = row![
        text(t!(i18n, ids::PCB_VIEW_DRAFT))
            .size(TS.caption)
            .color(muted),
        text(t!(i18n, ids::PCB_VIEW_TRANSPARENCY))
            .size(TS.caption)
            .color(muted)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right),
    ]
    .spacing(SP.s);
    let mut rows = vec![
        container(all_row).padding([0.0, SP.s]).boxed(),
        container(header).padding([0.0, SP.m]).boxed(),
    ];
    rows.extend(
        BoardObject::ALL
            .into_iter()
            .map(|kind| object_row(panel, kind, ctx)),
    );
    rows
}

/// One kind: its eye, Draft (not yet) and its transparency slider.
fn object_row<'a>(
    panel: &PcbLayersPanel,
    kind: BoardObject,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let shown = panel.shows_object(kind);
    let toggled = kind
        .can_hide()
        .then_some(PanelMsg::PcbLayers(PcbViewMsg::ObjectVisibilitySet {
            kind: Some(kind),
            visible: !shown,
        }));
    let ink = if shown && kind.can_hide() {
        crate::styles::ti(ctx.tokens.text)
    } else {
        resolve(&ctx.tokens).ink3
    };
    let draft = not_yet(
        checkbox(false)
            .size(TS.body)
            .style(form_look::checkbox_style)
            .boxed(),
        panel,
        ctx,
    );
    let percent = panel.paint.transparency(kind);
    let amount = text(t!(i18n, ids::PCB_VIEW_PERCENT, "percent" => u32::from(percent)))
        .size(TS.caption)
        .color(resolve(&ctx.tokens).ink3);
    let fade: Element<'a, PanelMsg> = if kind.can_fade() {
        slider(0..=100u8, percent, move |percent| {
            PanelMsg::PcbLayers(PcbViewMsg::ObjectTransparencySet { kind, percent })
        })
        .width(Length::Fill)
        .boxed()
    } else {
        not_yet(
            iced::widget::Space::new().width(Length::Fill).boxed(),
            panel,
            ctx,
        )
    };
    container(
        row![
            eye(panel, !shown, toggled, ctx),
            text(t!(i18n, ids::PCB_VIEW_OBJECT, "kind" => kind.token()))
                .size(TS.body)
                .color(ink)
                .width(Length::FillPortion(3)),
            draft,
            container(fade).width(Length::FillPortion(3)),
            amount,
        ]
        .spacing(SP.xs)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.s])
    .width(Length::Fill)
    .boxed()
}

/// Mask and Dim Settings: what the view draws today, greyed until it can be
/// changed.
fn mask_dim_rows<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    let dimmed = (DIMMED_ALPHA * 100.0).round() as u32;
    [
        (ids::PCB_VIEW_DIMMED, dimmed),
        (ids::PCB_VIEW_HIGHLIGHTED, 100),
        (ids::PCB_VIEW_MASKED, dimmed),
    ]
    .into_iter()
    .map(|(id, percent)| {
        let value = not_yet(
            text(t!(i18n, ids::PCB_VIEW_PERCENT, "percent" => percent))
                .size(TS.body)
                .color(resolve(&ctx.tokens).ink4)
                .into(),
            panel,
            ctx,
        );
        line(t!(i18n, id), value)
    })
    .collect()
}

/// The token of each Additional Options chip, in its order.
const OPTION_CHIPS: [&str; 11] = [
    "test_points",
    "status_info",
    "pad_nets",
    "pad_numbers",
    "via_nets",
    "via_span",
    "all_connections",
    "net_color_override",
    "layer_colors_for_connections",
    "repeated_net_names",
    "special_strings",
];

/// Additional Options: greyed chips until the view can draw them.
fn additional_chips<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let chips = OPTION_CHIPS.into_iter().map(|token| {
        not_yet(
            button(text(t!(panel.i18n, ids::PCB_VIEW_OPTION, "option" => token)).size(TS.caption))
                .padding([SP.xs, SP.s])
                .style(toggle_button(&ctx.tokens, false))
                .boxed(),
            panel,
            ctx,
        )
    });
    container(Row::with_children(chips).spacing(SP.xs).wrap())
        .padding([SP.xs, SP.m])
        .boxed()
}
