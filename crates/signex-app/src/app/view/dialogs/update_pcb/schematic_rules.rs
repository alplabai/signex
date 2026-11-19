//! The Rules from the schematic rows of Update PCB (#1281): each rule the
//! Net Parameters directives ask for, what Execute does to it (rewrite a
//! rule edited in the Rules tab, add a new one, or nothing), the rule as it
//! will read and, for a rewrite, as it reads now. Rewrites come first.

use iced::widget::{Column, column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use crate::app::Message;
use crate::app::update_pcb::rules_from_schematic::{RuleChange, SchematicRuleRow};
use crate::styles::{Spacing, TypeScale, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The section, or nothing when the schematic asks for no rule.
pub(super) fn section<'a>(
    rows: &[SchematicRuleRow],
    i18n: &I18n,
    theme: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    if rows.is_empty() {
        return None;
    }
    let muted = ti(theme.text_secondary);
    let ink = ti(theme.text);
    let r = tokens::resolve(theme);
    let mut list = Column::new().spacing(SP.xs).push(
        text(t!(i18n, ids::UPDATE_PCB_SCHEMATIC_RULES))
            .size(TS.label)
            .color(muted)
            .boxed(),
    );
    for row_data in rows {
        let (tag, tint) = match &row_data.change {
            RuleChange::Changed { .. } => (ids::UPDATE_PCB_RULE_CHANGED, r.warn_soft),
            RuleChange::Added => (ids::UPDATE_PCB_RULE_ADDED, r.ok_soft),
            RuleChange::Unchanged => (ids::UPDATE_PCB_RULE_UNCHANGED, r.sunken),
        };
        let pill = container(text(t!(i18n, tag)).size(TS.caption).color(ink))
            .padding([0.0, SP.s])
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(tint)),
                border: iced::Border {
                    radius: SP.s.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });
        let mut lines = column![
            row![
                text(UserText::new(row_data.rule.name.clone()))
                    .size(TS.body)
                    .color(ink)
                    .font(iced::Font::MONOSPACE)
                    .width(Length::Fill),
                pill,
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
            text(UserText::new(row_data.rule.text()))
                .size(TS.caption)
                .color(muted)
                .font(iced::Font::MONOSPACE),
        ]
        .spacing(SP.xs);
        if let RuleChange::Changed { was } = &row_data.change {
            lines = lines.push(
                text(t!(i18n, ids::UPDATE_PCB_RULE_WAS, "text" => was.as_str()))
                    .size(TS.caption)
                    .color(muted)
                    .font(iced::Font::MONOSPACE)
                    .boxed(),
            );
        }
        list = list.push(lines.boxed());
    }
    list = list.push(
        text(t!(i18n, ids::UPDATE_PCB_RULES_NOTE))
            .size(TS.caption)
            .color(muted)
            .boxed(),
    );
    Some(container(list).padding([SP.m, SP.xl]).boxed())
}
