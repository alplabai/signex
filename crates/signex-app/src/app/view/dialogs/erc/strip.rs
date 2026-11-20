//! The ERC dialog's source strip under the title (#2413): a pill naming where
//! the rules come from, the scope note (and the machine-rules notice), and
//! the severity count tiles.

use iced::widget::{Column, Row, Space, column, container};
use iced::{Alignment, Background, Element, Length, Theme, Widget};
use signex_erc::Severity;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::stat_tiles::{Density, StatTile, Tone, stat_tiles};
use signex_widgets::{raw, text};

use crate::app::{Message, Signex};
use crate::styles::{HAIRLINE, PillTone, pill, pill_colors, tokens};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The source pill's label: the project's own rules, or this machine's
/// "User Default" profile (no project, a project without rules, or one
/// whose rule set is from a newer Signex).
pub(super) fn source_label(stores_rules: bool) -> MessageId {
    if stores_rules {
        ids::DIALOG_ERC_SOURCE_PROJECT
    } else {
        ids::DIALOG_ERC_SOURCE_USER_DEFAULT
    }
}

/// The scope note beside the pill.
pub(super) fn scope_note(has_project: bool, stores_rules: bool, unreadable: bool) -> MessageId {
    match (has_project, stores_rules, unreadable) {
        (_, true, _) => ids::DIALOG_ERC_SCOPE_PROJECT,
        (true, false, true) => ids::DIALOG_ERC_SCOPE_NEWER_PROJECT,
        (true, false, false) => ids::DIALOG_ERC_SCOPE_PROJECT_WITHOUT_RULES,
        (false, false, _) => ids::DIALOG_ERC_SCOPE_USER_DEFAULT,
    }
}

/// Caption and number tone of a severity tile. Off counts are neutral.
pub(super) fn tile_look(severity: Severity) -> (MessageId, Tone) {
    match severity {
        Severity::Error => (ids::DIALOG_ERC_CHIP_ERRORS, Tone::Err),
        Severity::Warning => (ids::DIALOG_ERC_CHIP_WARNINGS, Tone::Warn),
        Severity::Info => (ids::DIALOG_ERC_CHIP_INFO, Tone::Info),
        Severity::Off => (ids::DIALOG_ERC_CHIP_OFF, Tone::Neutral),
    }
}

/// The four severity tiles over `counts`. Read-only: the rule list filters
/// by name and Changed only, not by severity, so a tile does not filter it.
fn severity_tiles<'a>(
    counts: [(Severity, usize); 4],
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let tiles: Vec<StatTile<'a, Message>> = counts
        .iter()
        .map(|(severity, count)| {
            let (caption, tone) = tile_look(*severity);
            let count = u32::try_from(*count).unwrap_or(u32::MAX);
            StatTile::count(count, t!(i18n, caption)).tone(tone)
        })
        .collect();
    stat_tiles(tiles, Density::Compact, tokens, TS.body, i18n)
}

impl Signex {
    /// The strip under the title: source pill, scope note (with the
    /// machine-rules notice when it applies) and the severity tiles, over a
    /// hairline.
    pub(super) fn view_erc_source_strip<'a>(
        &'a self,
        stores_rules: bool,
        theme_tokens: &ThemeTokens,
    ) -> Element<'a, Message> {
        let i18n: &I18n = &self.ui_state.i18n;
        let muted = crate::styles::ti(theme_tokens.text_secondary);
        let has_project = self.document_state.active_document_project().is_some();
        let unreadable = self.active_project_has_unreadable_rules();

        let pill_ink = pill_colors(theme_tokens, PillTone::Accent).1;
        let source = container(
            text(t!(i18n, source_label(stores_rules)))
                .size(TS.caption)
                .color(pill_ink),
        )
        .padding([SP.xs, SP.m])
        .style(pill(theme_tokens, PillTone::Accent));

        let mut note: Column<Element<'a, Message>> = column![
            text(t!(i18n, scope_note(has_project, stores_rules, unreadable)))
                .size(TS.caption)
                .color(muted)
        ]
        .spacing(SP.xs)
        .width(Length::FillPortion(1));
        // Editing the machine's rules while they are loading, or while the
        // User Default file cannot be read or written: say so here, not only
        // in the log.
        if !stores_rules && let Some(notice) = self.machine_rules_notice() {
            // raw: the machine-rules notice is built in English by the rule
            // profile loader, which also logs it (#2061).
            note = note.push(
                text(raw(notice))
                    .size(TS.caption)
                    .color(tokens::danger(theme_tokens))
                    .boxed(),
            );
        }

        let tiles = container(severity_tiles(
            self.erc_dialog_severity_counts(),
            theme_tokens,
            i18n,
        ))
        .width(Length::FillPortion(1));

        let line: Row<Element<'a, Message>> = Row::new()
            .push(source.boxed())
            .push(note.boxed())
            .push(tiles.boxed())
            .spacing(SP.m)
            .align_y(Alignment::Center);
        let rule_color = tokens::resolve(theme_tokens).line_soft;
        let rule =
            container(Space::new().width(Length::Fill).height(HAIRLINE)).style(move |_: &Theme| {
                container::Style {
                    background: Some(Background::Color(rule_color)),
                    ..container::Style::default()
                }
            });
        column![
            container(line).padding([SP.m, SP.xl]).width(Length::Fill),
            rule
        ]
        .width(Length::Fill)
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pill_names_the_project_only_when_it_stores_rules() {
        assert_eq!(source_label(true), ids::DIALOG_ERC_SOURCE_PROJECT);
        assert_eq!(source_label(false), ids::DIALOG_ERC_SOURCE_USER_DEFAULT);
    }

    #[test]
    fn the_scope_note_keeps_its_four_cases() {
        assert_eq!(scope_note(true, true, false), ids::DIALOG_ERC_SCOPE_PROJECT);
        assert_eq!(
            scope_note(true, false, true),
            ids::DIALOG_ERC_SCOPE_NEWER_PROJECT
        );
        assert_eq!(
            scope_note(true, false, false),
            ids::DIALOG_ERC_SCOPE_PROJECT_WITHOUT_RULES
        );
        assert_eq!(
            scope_note(false, false, false),
            ids::DIALOG_ERC_SCOPE_USER_DEFAULT
        );
    }

    #[test]
    fn tiles_tone_only_the_reporting_severities() {
        assert_eq!(tile_look(Severity::Error).1, Tone::Err);
        assert_eq!(tile_look(Severity::Warning).1, Tone::Warn);
        assert_eq!(tile_look(Severity::Info).1, Tone::Info);
        assert_eq!(tile_look(Severity::Off).1, Tone::Neutral);
        assert_eq!(tile_look(Severity::Off).0, ids::DIALOG_ERC_CHIP_OFF);
    }
}
