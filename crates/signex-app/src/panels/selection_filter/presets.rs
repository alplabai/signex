//! SCH Filter's Presets section (#56): a chip per saved preset (a click
//! applies it and makes it the one edited below), Save current, and the
//! edited preset's name, Delete and kinds. The list is the one the Active
//! Bar dropdown and Properties show (`active_bar::presets`).

use iced::widget::{Column, Row, container};
use iced::{Alignment, Element, Length, Widget as _};
use iced_aw::Wrap;
use signex_i18n::{I18n, ListKind, UserText, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{button, text, text_input};

use super::SchFilterMsg;
use crate::active_bar::{CUSTOM_FILTER_PRESET_LIMIT, CustomFilterPreset, preset_label};
use crate::panels::form_look::{ghost_button, text_input_style, value_field_button};
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::tokens::{hover_overlay, resolve};
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Collapse key of the section.
const SECTION_KEY: &str = "sch_filter_presets";

fn msg(m: SchFilterMsg) -> PanelMsg {
    PanelMsg::SchFilter(m)
}

/// The Presets section, collapsible like a Properties section.
pub(super) fn section<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    ctx.collapsible(
        SECTION_KEY,
        ids::PANELS_FILTER_PRESETS,
        &ctx.collapsed_sections,
        || body(ctx),
    )
    .width(Length::Fill)
    .boxed()
}

fn body<'a>(ctx: &'a PanelContext) -> Column<Element<'a, PanelMsg>> {
    let i18n = &*ctx.i18n;
    let presets = &ctx.custom_filter_presets;
    let mut column: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.s).width(Length::Fill);
    column = column.push(
        container(chips(ctx))
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    let muted = resolve(&ctx.tokens).ink3;
    let Some(preset) = presets.get(ctx.active_custom_filter_tab) else {
        let none = text(t!(i18n, ids::PANELS_FILTER_NO_PRESETS))
            .size(TS.caption)
            .color(muted);
        return column.push(container(none).padding([SP.xs, SP.m]).boxed());
    };
    let idx = ctx.active_custom_filter_tab;
    column = column.push(name_row(ctx, idx, preset));
    let kinds = text(kinds_line(i18n, idx, preset))
        .size(TS.caption)
        .color(muted);
    column.push(container(kinds).padding([0.0, SP.m]).boxed())
}

/// A chip per preset, the edited one lit, then Save current, inert at the
/// limit with why on hover.
fn chips<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let style = chip_style(ctx);
    let metrics = ChipMetrics::from_text_size(TS.body);
    let presets = &ctx.custom_filter_presets;
    let mut wrap = Wrap::new().spacing(SP.s).line_spacing(SP.s);
    for (idx, preset) in presets.iter().enumerate() {
        wrap = wrap.push(chip(
            UserText::new(preset_label(idx, preset)),
            None,
            idx == ctx.active_custom_filter_tab,
            style,
            metrics,
            i18n,
            Some(msg(SchFilterMsg::PresetApplied(idx))),
        ));
    }
    let room = presets.len() < CUSTOM_FILTER_PRESET_LIMIT;
    let save = chip(
        t!(i18n, ids::PANELS_FILTER_SAVE_CURRENT),
        None,
        false,
        style,
        metrics,
        i18n,
        room.then(|| msg(SchFilterMsg::PresetSaved)),
    );
    let why = if room {
        t!(i18n, ids::PANELS_FILTER_SAVE_CURRENT_TIP)
    } else {
        t!(i18n, ids::PANELS_FILTER_PRESET_LIMIT, "limit" => CUSTOM_FILTER_PRESET_LIMIT)
    };
    let save = hint_tooltip(
        save,
        why,
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    container(wrap.push(save)).width(Length::Fill).boxed()
}

fn chip_style(ctx: &PanelContext) -> ChipStyle {
    let r = resolve(&ctx.tokens);
    ChipStyle {
        fill: r.field,
        edge: ti(ctx.tokens.border),
        hover: hover_overlay(&ctx.tokens),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

/// Name | the preset's name, Delete. A click on the name opens it for
/// typing; Enter renames.
fn name_row<'a>(
    ctx: &'a PanelContext,
    idx: usize,
    preset: &CustomFilterPreset,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = resolve(&ctx.tokens).ink3;
    let typed = ctx
        .selection_filter
        .rename
        .as_ref()
        .filter(|(at, _)| *at == idx)
        .map(|(_, name)| name.clone());
    let field = match typed {
        Some(name) => text_input(t!(i18n, ids::PANELS_FILTERS_PRESET_NAME), name.clone())
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .on_input(|typed| msg(SchFilterMsg::PresetRenameTyped(typed)))
            .on_submit(msg(SchFilterMsg::PresetRenamed(idx, name.clone())))
            .style(text_input_style)
            .width(Length::Fill)
            .boxed(),
        None => button(text(UserText::new(preset_label(idx, preset))).size(TS.body))
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .on_press(msg(SchFilterMsg::PresetRenameStarted(idx)))
            .style(value_field_button)
            .boxed(),
    };
    let delete = button(text(t!(i18n, ids::PANELS_FILTERS_DELETE)).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(msg(SchFilterMsg::PresetDeleted(idx)))
        .style(ghost_button)
        .boxed();
    let label = text(t!(i18n, ids::PANELS_FILTER_PRESET_NAME_LABEL))
        .size(TS.body)
        .color(muted)
        .boxed();
    let line: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(label)
        .push(field)
        .push(delete);
    container(line)
        .padding([0.0, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// "Wiring: Wires, Buses, …": the preset and the kinds it turns on.
pub(super) fn kinds_line(
    i18n: &I18n,
    idx: usize,
    preset: &CustomFilterPreset,
) -> signex_i18n::Localized {
    let name = preset_label(idx, preset);
    if preset.filters.is_empty() {
        return t!(i18n, ids::PANELS_FILTER_PRESET_NO_KINDS, "name" => name);
    }
    let kinds: Vec<String> = preset
        .filters
        .iter()
        .map(|kind| t!(i18n, kind.message()).into_string())
        .collect();
    let kinds = i18n.list(&kinds, ListKind::And);
    t!(i18n, ids::PANELS_FILTER_PRESET_KINDS, "name" => name, "kinds" => kinds)
}
