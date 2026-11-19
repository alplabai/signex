//! The Presets side panel (#2398, #2530): three generic tier cards with
//! their key minimums, the rule profiles that hold DRC rules, and the
//! preview of a pick (Added / Changed / Removed, old → new) with Cancel and
//! Apply in the same card. Apply stores it as one undo step.

use std::sync::LazyLock;

use iced::widget::{Column, Row, Space, container, scrollable, svg};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_drc_dsl::table::presets::{RowChange, RowChangeKind};
use signex_i18n::{ids, t};
use signex_types::coord::Unit;
use signex_types::project::{FabTemplate, RuleProfile};
use signex_widgets::{LengthFormat, button, text, tooltip};

use super::{Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::presets::{change_values, profile_preset, tier_preset, tier_spec};
use crate::app::rules_tab::{PresetPreview, RulesTabMsg};
use crate::styles::{PillTone, Radii, ti, tokens, tokens::resolve};

static CLOSE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../../../assets/icons/close.svg").as_slice())
});

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

fn medium() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

fn tier_label(tier: FabTemplate) -> signex_i18n::MessageId {
    match tier {
        FabTemplate::Standard => ids::RULES_PRESET_STANDARD,
        FabTemplate::Advanced => ids::RULES_PRESET_ADVANCED,
        FabTemplate::Hdi => ids::RULES_PRESET_HDI,
    }
}

pub(super) fn view<'a>(
    frame: &Frame<'a>,
    profiles: Option<Vec<&'a RuleProfile>>,
) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let mut body: Column<Element<'a, Message>> = Column::new().spacing(SP.s).push(
        text(t!(i18n, ids::RULES_PRESETS_HINT))
            .size(TS.caption)
            .color(tokens::muted_text(tokens))
            .boxed(),
    );
    let picked = frame.state.preview.as_ref().map(|p| p.preset.as_str());
    for tier in FabTemplate::ALL {
        let preset = tier_preset(tier);
        let on = picked == Some(preset.as_str());
        body = body.push(tier_card(frame, tier, preset, on));
    }
    match profiles {
        None => {
            body = body.push(
                button(text(t!(i18n, ids::RULES_PRESETS_LOAD)).size(TS.caption))
                    .padding([SP.xs, SP.m])
                    .style(crate::styles::ghost_button(tokens))
                    .on_press(up(RulesTabMsg::PresetsListRequested { project_id: id }))
                    .boxed(),
            );
        }
        Some(profiles) if profiles.is_empty() => {
            body = body.push(
                text(t!(i18n, ids::RULES_PRESETS_NO_PROFILES))
                    .size(TS.caption)
                    .color(tokens::muted_text(tokens))
                    .boxed(),
            );
        }
        Some(profiles) => {
            for profile in profiles {
                let preset = profile_preset(&profile.name);
                let on = picked == Some(preset.as_str());
                let title = text(user(profile.name.clone()))
                    .size(TS.body)
                    .font(medium());
                body = body.push(card_button(frame, title.boxed(), preset, on));
            }
        }
    }
    if let Some(preview) = frame.state.preview.as_ref() {
        body = body.push(preview_card(frame, preview));
    }
    let close = tooltip(
        button(icon(&CLOSE, tokens))
            .padding(SP.xs)
            .style(crate::styles::ghost_button(tokens))
            .on_press(up(RulesTabMsg::PanelPicked {
                project_id: id,
                panel: None,
            })),
        text(t!(i18n, ids::RULES_PRESETS_CLOSE)).size(TS.caption),
        tooltip::Position::Bottom,
    );
    let header = container(
        Row::new()
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .push(
                text(t!(i18n, ids::RULES_PRESETS_TITLE))
                    .size(TS.label)
                    .font(medium())
                    .boxed(),
            )
            .push(Space::new().width(Length::Fill).boxed())
            .push(close.boxed()),
    )
    .padding([SP.s, SP.l])
    .width(Length::Fill);
    let line = ti(tokens.border);
    let panel_fill = ti(tokens.panel_bg);
    container(
        Column::new()
            .push(header.boxed())
            .push(
                scrollable(body.padding([SP.m, SP.l]))
                    .direction(crate::styles::thin_vertical())
                    .style(crate::styles::scrollbar_style(tokens))
                    .height(Length::Fill)
                    .boxed(),
            )
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(panel_fill)),
        border: Border {
            width: 1.0,
            color: line,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

fn icon<'a>(
    handle: &svg::Handle,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let ink = tokens::muted_text(tokens);
    svg(handle.clone())
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..svg::Style::default()
        })
        .boxed()
}

/// A tier card: its name and "clearance · width · drill · annular mm" in
/// mono, from the canonical tier table.
fn tier_card<'a>(
    frame: &Frame<'a>,
    tier: FabTemplate,
    preset: String,
    on: bool,
) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let format = LengthFormat::new(i18n, Unit::Mm);
    let spec = tier_spec(tier);
    let face = Column::new()
        .spacing(SP.xs)
        .push(
            text(t!(i18n, tier_label(tier)))
                .size(TS.body)
                .font(medium())
                .boxed(),
        )
        .push(
            text(t!(
                i18n,
                ids::RULES_PRESET_SPEC,
                "clearance" => format.number(spec.clearance),
                "width" => format.number(spec.width),
                "drill" => format.number(spec.drill),
                "annular" => format.number(spec.annular)
            ))
            .size(TS.caption)
            .font(mono())
            .color(tokens::muted_text(tokens))
            .boxed(),
        );
    card_button(frame, face.boxed(), preset, on)
}

/// A pickable card: the accent edge and wash when picked.
fn card_button<'a>(
    frame: &Frame<'a>,
    face: Element<'a, Message>,
    preset: String,
    on: bool,
) -> Element<'a, Message> {
    let (tokens, id) = (frame.tokens, frame.id);
    let r = resolve(tokens);
    let (fill, wash, ink) = (r.raised, r.accent_soft, ti(tokens.text));
    let edge = if on {
        ti(tokens.accent)
    } else {
        ti(tokens.border)
    };
    let hover = tokens::hover_overlay(tokens);
    button(face)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme, status| {
            let base = if on { wash } else { fill };
            let background = match status {
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                    if !on =>
                {
                    tokens::composite_over(hover, fill)
                }
                _ => base,
            };
            iced::widget::button::Style {
                background: Some(Background::Color(background)),
                text_color: ink,
                border: Border {
                    width: 1.0,
                    color: edge,
                    radius: Radii::GRAPHITE.card.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(up(RulesTabMsg::PresetPicked {
            project_id: id,
            preset,
        }))
        .boxed()
}

/// One change of the preview: pill, rule name in mono, old → new faint.
fn change_row<'a>(frame: &Frame<'a>, change: &RowChange) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let (word, tone) = match change.kind {
        RowChangeKind::Added => (ids::RULES_PRESET_PILL_ADDED, PillTone::Ok),
        RowChangeKind::Changed => (ids::RULES_PRESET_PILL_CHANGED, PillTone::Info),
        RowChangeKind::Removed => (ids::RULES_PRESET_PILL_REMOVED, PillTone::Err),
    };
    let pill = container(text(t!(i18n, word)).size(TS.caption))
        .padding([0.0, SP.s])
        .style(crate::styles::pill(tokens, tone));
    let values = match change_values(change) {
        (Some(from), Some(to)) => {
            Some(t!(i18n, ids::RULES_PRESET_FROM_TO, "from" => from, "to" => to).into_string())
        }
        (None, Some(value)) | (Some(value), None) => Some(value),
        (None, None) => None,
    };
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(pill.boxed())
        .push(
            text(user(change.name.clone()))
                .size(TS.caption)
                .font(mono())
                .boxed(),
        )
        .push(Space::new().width(Length::Fill).boxed());
    if let Some(values) = values {
        line = line.push(
            text(user(values))
                .size(TS.caption)
                .font(mono())
                .color(resolve(tokens).ink4)
                .boxed(),
        );
    }
    line.boxed()
}

/// "Applying <preset> changes:", one row per change, then Cancel / Apply.
fn preview_card<'a>(frame: &Frame<'a>, preview: &PresetPreview) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let label = match preview
        .preset
        .strip_prefix("tier:")
        .and_then(signex_drc_dsl::table::presets::tier_from_id)
    {
        Some(tier) => t!(i18n, tier_label(tier)).into_string(),
        None => preview.label.clone(),
    };
    let mut lines: Column<Element<'a, Message>> = Column::new().spacing(SP.xs).push(
        text(t!(i18n, ids::RULES_PRESET_PREVIEW, "preset" => label))
            .size(TS.body)
            .font(medium())
            .boxed(),
    );
    if preview.changes.is_empty() && preview.other.is_empty() {
        lines = lines.push(
            text(t!(i18n, ids::RULES_PRESET_NO_CHANGE))
                .size(TS.caption)
                .color(tokens::muted_text(tokens))
                .boxed(),
        );
    }
    for change in &preview.changes {
        lines = lines.push(change_row(frame, change));
    }
    for other in &preview.other {
        lines = lines.push(
            text(user(other.clone()))
                .size(TS.caption)
                .color(tokens::muted_text(tokens))
                .boxed(),
        );
    }
    let cancel = button(text(t!(i18n, ids::RULES_PRESET_CANCEL)).size(TS.caption))
        .padding([SP.xs, SP.m])
        .style(crate::styles::outline_button(tokens))
        .on_press(up(RulesTabMsg::PresetDismissed { project_id: id }));
    let apply = button(text(t!(i18n, ids::RULES_PRESET_APPLY)).size(TS.caption))
        .padding([SP.xs, SP.m])
        .style(crate::styles::primary_button(tokens))
        .on_press(up(RulesTabMsg::PresetApplied { project_id: id }));
    lines = lines.push(
        Row::new()
            .spacing(SP.s)
            .push(Space::new().width(Length::Fill).boxed())
            .push(cancel.boxed())
            .push(apply.boxed())
            .boxed(),
    );
    let r = resolve(tokens);
    let (fill, line) = (r.field, ti(tokens.border));
    container(lines)
        .padding(SP.m)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
