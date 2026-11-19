//! The View Options tab (#2363, mockup m103): the Configuration row, then
//! General Settings, Object Visibility, Mask and Dim Settings and the
//! Additional Options. Object Visibility and Mask and Dim share one grid of
//! four columns, so their values line up. A control the board view cannot
//! honour yet is greyed with a tooltip. Layout: `docs/PCB_TOOLS.md`,
//! "Board view configuration".

mod extras;
mod general;
mod objects;

use iced::widget::{Column, Row, Space, container, row, slider};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget};
use signex_i18n::{Localized, MessageId, ids, t};
use signex_widgets::{CheckboxLabel as _, checkbox, text};

use super::rows::{SectionHead, section_with};
use super::{PcbLayersPanel, SP, TS};
use crate::panels::form_look::{self, FormLook};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ChromeMetrics, Radii, chrome_px, tokens::resolve};

/// The eye column of the grid: a square row cell, wide enough for an eye
/// button.
const EYE_W: f32 = ChromeMetrics::DEFAULT.row;
/// The Draft column.
const DRAFT_W: f32 = chrome_px(40.0 * 11.0 / 12.0);
/// The Transparency column: a slider and its percent.
const FADE_W: f32 = chrome_px(96.0 * 11.0 / 12.0);
/// The percent after a slider, right-aligned.
const PERCENT_W: f32 = chrome_px(34.0 * 11.0 / 12.0);
/// A General Settings or Additional Options row: a field's height and a
/// hairline gap.
const LINE_H: f32 = ChromeMetrics::DEFAULT.field + SP.xs;
/// The grid's header row, shorter than a row.
const HEAD_H: f32 = chrome_px(20.0 * 11.0 / 12.0);
/// The rail of a slider, and the diameter of its handle.
const RAIL_H: f32 = chrome_px(4.0 * 11.0 / 12.0);
const HANDLE: f32 = chrome_px(12.0 * 11.0 / 12.0);

pub(super) fn view<'a>(panel: &'a PcbLayersPanel, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let sections = [
        open_section(
            ctx,
            panel,
            "pcb_view_general",
            ids::PCB_VIEW_GENERAL,
            general::rows(panel, ctx),
        ),
        open_section(
            ctx,
            panel,
            "pcb_view_objects",
            ids::PCB_VIEW_OBJECTS,
            objects::rows(panel, ctx),
        ),
        open_section(
            ctx,
            panel,
            "pcb_view_mask_dim",
            ids::PCB_VIEW_MASK_DIM,
            objects::mask_dim_rows(panel, ctx),
        ),
        extras::section(panel, ctx),
    ];
    let column = Column::<Element<'a, PanelMsg>>::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(general::configuration(panel, ctx));
    sections
        .into_iter()
        .flatten()
        .fold(column, |col, section| col.push(section))
        .boxed()
}

/// A section open by default, with no count and no eye in its header.
fn open_section<'a>(
    ctx: &'a PanelContext,
    panel: &PcbLayersPanel,
    key: &str,
    title: MessageId,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    let head = SectionHead {
        count: None,
        eye: None,
        closed_by_default: false,
    };
    section_with(ctx, panel, key, title, head, rows)
}

/// The four cells of a grid row: the eye, the name, Draft and Transparency.
struct GridCells<'a> {
    eye: Element<'a, PanelMsg>,
    name: Element<'a, PanelMsg>,
    draft: Element<'a, PanelMsg>,
    fade: Element<'a, PanelMsg>,
}

/// One row of the Object Visibility grid, `height` tall, inside the
/// panel's side padding.
fn grid_row<'a>(cells: GridCells<'a>, height: f32) -> container::Container<'a, PanelMsg> {
    let GridCells {
        eye,
        name,
        draft,
        fade,
    } = cells;
    let line = row![
        container(eye).center_x(EYE_W),
        container(name).width(Length::Fill),
        container(draft).center_x(DRAFT_W),
        container(fade).width(FADE_W),
    ]
    .spacing(SP.s)
    .height(height)
    .align_y(Alignment::Center);
    container(line)
        .padding(Padding {
            top: 0.0,
            right: SP.m,
            bottom: 0.0,
            left: SP.m,
        })
        .width(Length::Fill)
}

/// An empty grid cell.
fn blank<'a>() -> Element<'a, PanelMsg> {
    Space::new().boxed()
}

/// The percent after a slider: mono caption, right-aligned.
fn percent<'a>(panel: &PcbLayersPanel, ctx: &PanelContext, value: u32) -> Element<'a, PanelMsg> {
    text(t!(panel.i18n, ids::PCB_VIEW_PERCENT, "percent" => value))
        .size(TS.caption)
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .color(resolve(&ctx.tokens).ink3)
        .width(PERCENT_W)
        .align_x(iced::alignment::Horizontal::Right)
        .boxed()
}

/// The Transparency cell: a slider, then its percent.
fn fade_cell<'a>(
    slider: Element<'a, PanelMsg>,
    percent: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![slider, percent]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .boxed()
}

/// A working transparency slider: the accent rail up to the handle, or the
/// faint ink while its kind is hidden.
fn live_slider<'a>(
    value: u8,
    faint: bool,
    on_change: impl Fn(u8) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    slider(0..=100u8, value, on_change)
        .width(Length::Fill)
        .height(HANDLE)
        .style(move |theme: &Theme, _: slider::Status| {
            let look = FormLook::of(theme);
            let ink = if faint { look.ink4 } else { look.accent };
            slider::Style {
                rail: slider::Rail {
                    backgrounds: (Background::Color(ink), Background::Color(look.line_strong)),
                    width: RAIL_H,
                    border: Border {
                        radius: (RAIL_H / 2.0).into(),
                        ..Border::default()
                    },
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle {
                        radius: HANDLE / 2.0,
                    },
                    background: Background::Color(look.raised),
                    border_width: 1.0,
                    border_color: ink,
                },
            }
        })
        .boxed()
}

/// A slider the view cannot move yet: the rail filled to `value` and the
/// handle, in the faint ink, with no message.
fn static_slider<'a>(ctx: &PanelContext, value: u32) -> Element<'a, PanelMsg> {
    let tokens = resolve(&ctx.tokens);
    let (ink, rail, knob) = (tokens.ink4, tokens.line_strong, tokens.raised);
    let bar = |colour: Color, portion: u16| {
        container(Space::new())
            .width(Length::FillPortion(portion))
            .height(RAIL_H)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(colour)),
                border: Border {
                    radius: (RAIL_H / 2.0).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    };
    let handle = container(Space::new())
        .width(HANDLE)
        .height(HANDLE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(knob)),
            border: Border {
                width: 1.0,
                color: ink,
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed();
    // An empty or full rail leaves that side out: a zero portion would
    // still be laid out.
    let filled = value.min(100) as u16;
    let mut line = Row::new().align_y(Alignment::Center).width(Length::Fill);
    if filled > 0 {
        line = line.push(bar(ink, filled));
    }
    line = line.push(handle);
    if filled < 100 {
        line = line.push(bar(rail, 100 - filled));
    }
    line.boxed()
}

/// A checkbox the view cannot honour yet: unticked, faint, no message.
fn greyed_checkbox<'a>(label: Option<Localized>) -> Element<'a, PanelMsg> {
    let mut tick = checkbox(false)
        .size(TS.body)
        .style(|theme: &Theme, status| {
            let mut style = form_look::checkbox_style(theme, status);
            let look = FormLook::of(theme);
            style.text_color = Some(look.ink4);
            style.border.color = look.line;
            style
        });
    if let Some(label) = label {
        tick = tick.labelled(label).text_size(TS.body).spacing(SP.s);
    }
    tick.boxed()
}
