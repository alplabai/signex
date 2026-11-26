//! The Variant Manager's toolbar (active variant, parts filter, Only rows
//! that differ, Settings not built yet, Compare variants and Open in BOM
//! Studio) and its footer (#1293, #2949), the slots of the shared
//! document-tab shell.

use iced::widget::{container, pick_list};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::doc_tab_shell::{Footer, Toolbar};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, text, text_input};

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{ButtonKind, text_button};
use super::super::table::{SP, TS};
use super::{Manager, VariantManagerMsg};
use crate::app::Message;
use crate::choice::Choice;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Width of the Active variant select, in `body`-size characters.
const SELECT_EMS: f32 = 128.0 / 12.0;
/// Width of the parts filter, in `body`-size characters.
const FILTER_EMS: f32 = 190.0 / 12.0;

/// Caption, Active variant select, separator, parts filter, Only rows that
/// differ; at the end Read again while the parts are older than the sheets,
/// then Settings, greyed with a "not built yet" tooltip (its surface has no
/// mockup yet), Compare variants and Open in BOM Studio (#2949).
pub(super) fn toolbar<'a>(m: &Manager<'_>) -> Toolbar<'a, Message> {
    let r = resolve(m.tokens);
    let mut bar = Toolbar::new()
        .push(
            text(t!(m.i18n, ids::PROJECT_VARIANTS_ACTIVE_CAPTION))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        )
        .push(active_picker(m))
        .separator()
        .push(filter(m))
        .push(only_differ(m));
    if m.stale {
        bar = bar.push_end(read_again(m));
    }
    if !m.definitions.is_empty() {
        for id in NOT_BUILT {
            bar = bar.push_end(not_built(m, id));
        }
        for (id, msg) in actions(m) {
            let press = tab_message(ProjectTabMsg::VariantManager {
                project_id: m.project_id,
                msg,
            });
            bar = bar.push_end(text_button(
                None,
                t!(m.i18n, id),
                Some(press),
                ButtonKind::Outline,
                m.tokens,
            ));
        }
    }
    bar
}

/// The top bar actions that are built (#2949) and what each sends: Compare
/// variants shows the comparison, Open in BOM Studio opens the BOM with the
/// variant selected in the list.
pub(super) fn actions(m: &Manager<'_>) -> [(MessageId, VariantManagerMsg); 2] {
    [
        (
            ids::VARIANT_MANAGER_COMPARE,
            VariantManagerMsg::CompareOpened,
        ),
        (
            ids::VARIANT_MANAGER_OPEN_BOM,
            VariantManagerMsg::BomRequested {
                variant: m.selected.map(str::to_string),
            },
        ),
    ]
}

/// The Active variant select: Base, then each definition.
fn active_picker<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let project_id = m.project_id;
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
    pick_list(selected, choices, |choice: &Choice<Option<String>>| {
        choice.label.clone()
    })
    .graphite()
    .on_select(move |choice: Choice<Option<String>>| {
        tab_message(ProjectTabMsg::ActiveVariantPicked {
            project_id,
            name: choice.value,
        })
    })
    .width(Length::Fixed(TS.body * SELECT_EMS))
    .boxed()
}

/// The parts filter.
fn filter<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let project_id = m.project_id;
    let filter_text = m
        .tab
        .map_or_else(String::new, |tab| tab.variant_filter.clone());
    text_input(t!(m.i18n, ids::PROJECT_VARIANTS_FILTER), filter_text)
        .on_input(move |text| tab_message(ProjectTabMsg::VariantFilterEdited { project_id, text }))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fixed(TS.body * FILTER_EMS))
        .boxed()
}

/// The Only rows that differ chip with its count.
fn only_differ<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let r = resolve(m.tokens);
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
    chip(
        t!(m.i18n, ids::PROJECT_VARIANTS_ONLY_DIFFER),
        count,
        only_differ,
        style,
        ChipMetrics::from_text_size(TS.body),
        m.i18n,
        Some(tab_message(ProjectTabMsg::VariantDifferingToggled {
            project_id: m.project_id,
        })),
    )
}

/// The top bar actions whose surfaces are not built yet (mockup row 32):
/// Settings. Its variant defaults are `project_set_default_variant`; the
/// surface waits for a mockup (#2949), see `docs/PROJECT_TAB.md`.
pub(super) const NOT_BUILT: [MessageId; 1] = [ids::VARIANT_MANAGER_SETTINGS];

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

/// Read again, shown while the parts are older than the sheets.
fn read_again<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    button(text(t!(m.i18n, ids::PROJECT_VARIANTS_READ_AGAIN)).size(TS.caption))
        .padding([0.0, SP.m])
        .style(crate::styles::ghost_button(m.tokens))
        .on_press(tab_message(ProjectTabMsg::VariantPartsReadRequested {
            project_id: m.project_id,
        }))
        .boxed()
}

/// The footer: parts, variants and cells that differ from Base, and the
/// note that the parts are older than the sheets while they are.
pub(super) fn footer<'a>(m: &Manager<'_>) -> Footer<'a> {
    let words = m.matrix().map(|matrix| {
        let parts = matrix.rows.len();
        if m.definitions.is_empty() {
            t!(m.i18n, ids::PROJECT_VARIANTS_STATUS_BASE, "parts" => parts)
        } else {
            t!(
                m.i18n,
                ids::PROJECT_VARIANTS_STATUS,
                "parts" => parts,
                "variants" => m.definitions.len() + 1,
                "cells" => matrix.differing_cells(),
            )
        }
    });
    let footer = match words {
        Some(words) => Footer::new(words),
        None => Footer::default(),
    };
    if m.stale {
        footer.note(t!(m.i18n, ids::PROJECT_VARIANTS_STALE))
    } else {
        footer
    }
}
