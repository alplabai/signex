//! Object Visibility (All Objects, then one row per kind) and Mask and Dim
//! Settings, on the same four columns.

use iced::widget::tooltip::Position;
use iced::widget::{container, svg};
use iced::{Element, Length, Theme, Widget};
use signex_i18n::{MessageId, ids, t};
use signex_renderer::pcb::{BoardObject, DIMMED_ALPHA};
use signex_widgets::{button, text};

use super::super::metrics::{EYE, ICON};
use super::super::rows::{eye, not_yet};
use super::super::{PcbLayersPanel, TS, eye_handle, row_button};
use super::{
    GridCells, HEAD_H, blank, fade_cell, greyed_checkbox, grid_row, live_slider, percent,
    static_slider,
};
use crate::app::PcbViewMsg;
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ChromeMetrics, HAIRLINE, ti, tokens::resolve};

/// Alpha of the All Objects eye while the kinds differ.
const MIXED_EYE_ALPHA: f32 = 0.6;

/// Whether every kind the view can hide is shown (`Some(true)`), every one
/// hidden (`Some(false)`), or they differ (`None`: Mixed).
fn all_objects_shown(panel: &PcbLayersPanel) -> Option<bool> {
    let mut kinds = BoardObject::ALL.into_iter().filter(|k| k.can_hide());
    let first = panel.shows_object(kinds.next()?);
    kinds
        .all(|kind| panel.shows_object(kind) == first)
        .then_some(first)
}

/// Object Visibility: the column header, All Objects, then one row per
/// kind.
pub(super) fn rows<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Vec<Element<'a, PanelMsg>> {
    let mut rows = vec![header(panel, ctx), all_objects_row(panel, ctx)];
    rows.extend(
        BoardObject::ALL
            .into_iter()
            .map(|kind| object_row(panel, kind, ctx)),
    );
    rows
}

/// The column names: a blank over the eyes, then Object, Draft and
/// Transparency.
fn header<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let ink = resolve(&ctx.tokens).ink4;
    let name = |id: MessageId| -> Element<'a, PanelMsg> {
        text(t!(panel.i18n, id)).size(TS.caption).color(ink).boxed()
    };
    grid_row(
        GridCells {
            eye: blank(),
            name: name(ids::PCB_VIEW_OBJECT_COLUMN),
            draft: name(ids::PCB_VIEW_DRAFT),
            fade: name(ids::PCB_VIEW_TRANSPARENCY),
        },
        HEAD_H,
    )
    .boxed()
}

/// All Objects, in medium weight over a hairline: its eye hides every kind,
/// or shows them all while one is hidden. While the kinds differ the eye
/// is faded and Transparency reads Mixed.
fn all_objects_row<'a>(panel: &PcbLayersPanel, ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let all = all_objects_shown(panel);
    let toggled = PanelMsg::PcbLayers(PcbViewMsg::ObjectVisibilitySet {
        kind: None,
        visible: all != Some(true),
    });
    let (eye_cell, fade) = match all {
        Some(shown) => (eye(panel, !shown, Some(toggled), ctx), blank()),
        None => (
            mixed_eye(panel, toggled, ctx),
            text(t!(i18n, ids::PCB_VIEW_MIXED))
                .size(TS.caption)
                .color(resolve(&ctx.tokens).ink4)
                .boxed(),
        ),
    };
    let name = text(t!(i18n, ids::PCB_VIEW_OBJECT, "kind" => "all"))
        .size(TS.body)
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
        })
        .color(ti(ctx.tokens.text))
        .boxed();
    let rule = resolve(&ctx.tokens).line_soft;
    grid_row(
        GridCells {
            eye: eye_cell,
            name,
            draft: not_yet(greyed_checkbox(None), panel, ctx),
            fade,
        },
        ChromeMetrics::DEFAULT.row,
    )
    .style(move |_: &Theme| {
        let mut border = iced::Border::default();
        border.bottom = iced::border::Side::default().width(HAIRLINE).color(rule);
        container::Style {
            border,
            ..container::Style::default()
        }
    })
    .boxed()
}

/// The All Objects eye while the kinds differ: open and faded, "Mixed" in
/// its tooltip; a click shows every kind.
fn mixed_eye<'a>(
    panel: &PcbLayersPanel,
    on_press: PanelMsg,
    ctx: &PanelContext,
) -> Element<'a, PanelMsg> {
    let ink = iced::Color {
        a: MIXED_EYE_ALPHA,
        ..ti(ctx.tokens.text)
    };
    // Drawn as `rows::eye` is: an ICON glyph centred in an EYE square.
    let icon = svg(eye_handle(false))
        .width(ICON)
        .height(ICON)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let eye = button(container(icon).center_x(EYE).center_y(EYE))
        .padding(0)
        .style(row_button(&ctx.tokens))
        .on_press(on_press)
        .boxed();
    hint_tooltip(
        eye,
        t!(panel.i18n, ids::PCB_VIEW_MIXED),
        &ctx.tokens,
        Position::Top,
    )
}

/// One kind: its eye (greyed where the view cannot hide it), its name
/// (muted while hidden), Draft (not yet) and its transparency: a slider
/// where the view can fade it, a greyed one where it cannot.
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
        ti(ctx.tokens.text)
    } else {
        resolve(&ctx.tokens).ink3
    };
    let value = panel.paint.transparency(kind);
    let slider = if kind.can_fade() {
        live_slider(value, !shown, move |percent| {
            PanelMsg::PcbLayers(PcbViewMsg::ObjectTransparencySet { kind, percent })
        })
    } else {
        not_yet(static_slider(ctx, u32::from(value)), panel, ctx)
    };
    grid_row(
        GridCells {
            eye: eye(panel, !shown, toggled, ctx),
            name: text(t!(i18n, ids::PCB_VIEW_OBJECT, "kind" => kind.token()))
                .size(TS.body)
                .color(ink)
                .width(Length::Fill)
                .boxed(),
            draft: not_yet(greyed_checkbox(None), panel, ctx),
            fade: fade_cell(slider, percent(panel, ctx, u32::from(value))),
        },
        ChromeMetrics::DEFAULT.row,
    )
    .boxed()
}

/// Mask and Dim Settings: what the view draws today, on the object rows'
/// columns, greyed until it can be changed.
pub(super) fn mask_dim_rows<'a>(
    panel: &PcbLayersPanel,
    ctx: &PanelContext,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = &panel.i18n;
    let ink = ti(ctx.tokens.text);
    let dimmed = (DIMMED_ALPHA * 100.0).round() as u32;
    [
        (ids::PCB_VIEW_DIMMED, dimmed),
        (ids::PCB_VIEW_HIGHLIGHTED, 100),
        (ids::PCB_VIEW_MASKED, dimmed),
    ]
    .into_iter()
    .map(|(id, value)| {
        grid_row(
            GridCells {
                eye: blank(),
                name: text(t!(i18n, id)).size(TS.body).color(ink).boxed(),
                draft: blank(),
                fade: fade_cell(
                    not_yet(static_slider(ctx, value), panel, ctx),
                    percent(panel, ctx, value),
                ),
            },
            ChromeMetrics::DEFAULT.row,
        )
        .boxed()
    })
    .collect()
}
