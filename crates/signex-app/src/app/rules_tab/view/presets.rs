//! The Presets panel (#2398): the generic manufacturing tiers and the rule
//! profiles that hold DRC rules; a pick shows the rules it adds, changes
//! and removes, and Apply stores them as one undo step.

use iced::widget::{Column, Row, column, row, scrollable};
use iced::{Element, Length, Widget as _};
use signex_drc_dsl::table::presets::RowChangeKind;
use signex_i18n::{ids, t};
use signex_types::project::{FabTemplate, RuleProfile};
use signex_widgets::{button, text};

use super::{Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::presets::{profile_preset, tier_preset};
use crate::app::rules_tab::{PresetPreview, RulesTabMsg};
use crate::styles::tokens;

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
    let mut body: Column<Element<'a, Message>> = Column::new().spacing(SP.s);
    body = body.push(
        text(t!(i18n, ids::RULES_PRESETS_TITLE))
            .size(TS.label)
            .boxed(),
    );
    body = body.push(
        text(t!(i18n, ids::RULES_PRESETS_HINT))
            .size(TS.caption)
            .color(tokens::muted_text(tokens))
            .boxed(),
    );
    let mut tiers: Row<Element<'a, Message>> = Row::new().spacing(SP.xs);
    for tier in FabTemplate::ALL {
        tiers = tiers.push(
            button(text(t!(i18n, tier_label(tier))).size(TS.caption))
                .padding([SP.xs, SP.s])
                .style(crate::styles::outline_button(tokens))
                .on_press(up(RulesTabMsg::PresetPicked {
                    project_id: id,
                    preset: tier_preset(tier),
                }))
                .boxed(),
        );
    }
    body = body.push(tiers.boxed());
    match profiles {
        None => {
            body = body.push(
                button(text(t!(i18n, ids::RULES_PRESETS_LOAD)).size(TS.caption))
                    .padding([SP.xs, SP.s])
                    .style(crate::styles::subtle_button(tokens))
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
            let mut list: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
            for profile in profiles {
                list = list.push(
                    button(text(user(profile.name.clone())).size(TS.caption))
                        .padding([SP.xs, SP.s])
                        .style(crate::styles::outline_button(tokens))
                        .on_press(up(RulesTabMsg::PresetPicked {
                            project_id: id,
                            preset: profile_preset(&profile.name),
                        }))
                        .boxed(),
                );
            }
            body = body.push(list.boxed());
        }
    }
    if let Some(preview) = frame.state.preview.as_ref() {
        body = body.push(preview_view(frame, preview));
    }
    body.width(Length::FillPortion(2)).boxed()
}

fn preview_view<'a>(frame: &Frame<'a>, preview: &PresetPreview) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let label = match preview
        .preset
        .strip_prefix("tier:")
        .and_then(signex_drc_dsl::table::presets::tier_from_id)
    {
        Some(tier) => t!(i18n, tier_label(tier)).into_string(),
        None => preview.label.clone(),
    };
    let mut lines: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    lines = lines.push(
        text(t!(i18n, ids::RULES_PRESET_PREVIEW, "preset" => label))
            .size(TS.body)
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
        let (word, ink) = match change.kind {
            RowChangeKind::Added => (ids::RULES_PRESET_ADDED, tokens::muted_text(tokens)),
            RowChangeKind::Changed => (ids::RULES_PRESET_CHANGED, tokens::muted_text(tokens)),
            RowChangeKind::Removed => (ids::RULES_PRESET_REMOVED, tokens::danger(tokens)),
        };
        lines = lines.push(
            text(t!(i18n, word, "rule" => change.name.as_str()))
                .size(TS.caption)
                .color(ink)
                .boxed(),
        );
        let shown = change.to.clone().or_else(|| change.from.clone());
        if let Some(rule) = shown {
            lines = lines.push(text(user(rule)).size(TS.caption).boxed());
        }
    }
    for other in &preview.other {
        lines = lines.push(text(user(other.clone())).size(TS.caption).boxed());
    }
    let apply = button(text(t!(i18n, ids::RULES_PRESET_APPLY)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::accent_button(tokens))
        .on_press(up(RulesTabMsg::PresetApplied { project_id: id }));
    let cancel = button(text(t!(i18n, ids::RULES_PRESET_CANCEL)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::subtle_button(tokens))
        .on_press(up(RulesTabMsg::PresetDismissed { project_id: id }));
    column![
        scrollable(lines).height(Length::Fixed(TS.body * 18.0)),
        row![apply, cancel].spacing(SP.s),
    ]
    .spacing(SP.s)
    .boxed()
}
