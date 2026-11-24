//! The Variant Manager's top bar (active variant, parts filter, Only rows
//! that differ) and its status line (#1293).

use iced::widget::{Row, Space, container, pick_list, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, text, text_input};

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{ButtonKind, text_button};
use super::super::table::{SP, TS};
use super::Manager;
use crate::app::Message;
use crate::choice::Choice;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Width of the Active variant select, in `body`-size characters.
const SELECT_EMS: f32 = 128.0 / 12.0;
/// Width of the parts filter, in `body`-size characters.
const FILTER_EMS: f32 = 190.0 / 12.0;
/// Height of the bar's vertical rule, in `body`-size characters.
const RULE_EMS: f32 = 20.0 / 12.0;
/// Height of the status line, in `body`-size characters.
const STATUS_EMS: f32 = 30.0 / 12.0;

/// Caption, Active variant select, rule, parts filter, Only rows that differ,
/// then Settings, Compare variants and Open in BOM Studio at the end, greyed
/// with a "not built yet" tooltip: none of their surfaces exists yet.
pub(super) fn top<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let project_id = m.project_id;
    let r = resolve(m.tokens);
    let choices: Vec<Choice<Option<String>>> = m
        .variants()
        .map(|variant| Choice {
            value: variant.map(str::to_string),
            label: m.name(variant),
        })
        .collect();
    let selected = choices
        .iter()
        .find(|choice| choice.value.as_deref() == m.active)
        .cloned();
    let picker = pick_list(selected, choices, |choice: &Choice<Option<String>>| {
        choice.label.clone()
    })
    .graphite()
    .on_select(move |choice: Choice<Option<String>>| {
        tab_message(ProjectTabMsg::ActiveVariantPicked {
            project_id,
            name: choice.value,
        })
    })
    .width(Length::Fixed(TS.body * SELECT_EMS));
    let filter_text = m
        .tab
        .map_or_else(String::new, |tab| tab.variant_filter.clone());
    let filter = text_input(t!(m.i18n, ids::PROJECT_VARIANTS_FILTER), filter_text)
        .on_input(move |text| tab_message(ProjectTabMsg::VariantFilterEdited { project_id, text }))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fixed(TS.body * FILTER_EMS));
    let only_differ = m.tab.is_some_and(|tab| tab.variants_only_differ);
    let count = m
        .matrix()
        .map(|matrix| u32::try_from(matrix.differing_rows()).unwrap_or(u32::MAX));
    let style = ChipStyle {
        fill: r.field,
        edge: ti(m.tokens.border),
        hover: ti(m.tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    let differ = chip(
        t!(m.i18n, ids::PROJECT_VARIANTS_ONLY_DIFFER),
        count,
        only_differ,
        style,
        ChipMetrics::from_text_size(TS.body),
        m.i18n,
        Some(tab_message(ProjectTabMsg::VariantDifferingToggled {
            project_id,
        })),
    );
    let line = r.line_soft;
    let mut bar = row![
        text(t!(m.i18n, ids::PROJECT_VARIANTS_ACTIVE_CAPTION))
            .size(TS.caption)
            .color(r.ink3),
        picker,
        rule(line),
        filter,
        differ,
        Space::new().width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    if !m.definitions.is_empty() {
        for id in NOT_BUILT {
            bar = bar.push(not_built(m, id));
        }
    }
    strip(bar, false, Length::Shrink, m)
}

/// The top bar actions whose surfaces are not built yet (mockup row 32).
pub(super) const NOT_BUILT: [MessageId; 3] = [
    ids::VARIANT_MANAGER_SETTINGS,
    ids::VARIANT_MANAGER_COMPARE,
    ids::VARIANT_MANAGER_OPEN_BOM,
];

/// A greyed top bar button with the "not built yet" tooltip.
fn not_built<'a>(m: &Manager<'_>, id: MessageId) -> Element<'a, Message> {
    let face = text_button(None, t!(m.i18n, id), None, ButtonKind::Outline, m.tokens);
    let tip = container(
        text(t!(m.i18n, ids::VARIANT_MANAGER_NOT_BUILT))
            .size(TS.caption)
            .color(ti(m.tokens.text)),
    )
    .padding([SP.xs, SP.s])
    .style(crate::styles::context_menu(m.tokens));
    signex_widgets::tooltip(face, tip, iced::widget::tooltip::Position::Bottom)
        .gap(SP.xs)
        .boxed()
}

/// The status line: parts, variants and cells that differ from Base, and a
/// Read again when the parts are older than the sheets.
pub(super) fn status<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.l).align_y(Alignment::Center);
    if let Some(matrix) = m.matrix() {
        let parts = matrix.rows.len();
        let words = if m.definitions.is_empty() {
            t!(m.i18n, ids::PROJECT_VARIANTS_STATUS_BASE, "parts" => parts)
        } else {
            t!(
                m.i18n,
                ids::PROJECT_VARIANTS_STATUS,
                "parts" => parts,
                "variants" => m.definitions.len() + 1,
                "cells" => matrix.differing_cells(),
            )
        };
        line = line.push(text(words).size(TS.caption).color(r.ink3).boxed());
    }
    line = line.push(Space::new().width(Length::Fill).boxed());
    if m.stale {
        let project_id = m.project_id;
        line = line.push(
            text(t!(m.i18n, ids::PROJECT_VARIANTS_STALE))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
        line = line.push(
            button(text(t!(m.i18n, ids::PROJECT_VARIANTS_READ_AGAIN)).size(TS.caption))
                .padding([0.0, SP.m])
                .style(crate::styles::ghost_button(m.tokens))
                .on_press(tab_message(ProjectTabMsg::VariantPartsReadRequested {
                    project_id,
                }))
                .boxed(),
        );
    }
    strip(line, true, Length::Fixed(TS.body * STATUS_EMS), m)
}

/// A full-width strip on the panel, `height` tall, a hairline below (the
/// bar) or above (the status line).
fn strip<'a>(
    content: Row<Element<'a, Message>>,
    above: bool,
    height: Length,
    m: &Manager<'_>,
) -> Element<'a, Message> {
    let panel = ti(m.tokens.panel_bg);
    let line = if above {
        ti(m.tokens.border)
    } else {
        resolve(m.tokens).line_soft
    };
    let side = iced::border::Side::default().width(1.0).color(line);
    let border = if above {
        Border::default().top(side)
    } else {
        Border::default().bottom(side)
    };
    container(content)
        .width(Length::Fill)
        .height(height)
        .padding([SP.s, SP.l])
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(panel)),
            border,
            ..container::Style::default()
        })
        .boxed()
}

/// A short vertical hairline between bar groups.
fn rule<'a>(color: iced::Color) -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(TS.body * RULE_EMS))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}
