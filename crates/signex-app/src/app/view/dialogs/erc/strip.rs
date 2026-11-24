//! The ERC dialog's source strip under the title (#2413): a pill naming where
//! the rules come from, the scope note (and the machine-rules notice), and
//! the severity count chips.

use iced::widget::{Column, Row, Space, column, container};
use iced::{Alignment, Background, Element, Length, Theme, Widget};
use signex_erc::Severity;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, ChipTone, tag_count};
use signex_widgets::{Label, raw, text, theme_ext};

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

/// Caption, tag letter and tag tone of a severity count chip. Off is
/// neutral, and its tag is a dash (`None`).
pub(super) fn chip_look(severity: Severity) -> (MessageId, Option<MessageId>, PillTone) {
    match severity {
        Severity::Error => (
            ids::DIALOG_ERC_CHIP_ERRORS,
            Some(ids::DIALOG_RULES_ERROR_SHORT),
            PillTone::Err,
        ),
        Severity::Warning => (
            ids::DIALOG_ERC_CHIP_WARNINGS,
            Some(ids::DIALOG_RULES_WARNING_SHORT),
            PillTone::Warn,
        ),
        Severity::Info => (
            ids::DIALOG_ERC_CHIP_INFO,
            Some(ids::DIALOG_RULES_INFO_SHORT),
            PillTone::Info,
        ),
        Severity::Off => (ids::DIALOG_ERC_CHIP_OFF, None, PillTone::Neutral),
    }
}

/// The four severity counts as compact chips, each led by its severity tag
/// (mockup row 84). Read-only: the rule list filters by name and Changed
/// only, not by severity, so a chip does not filter it.
pub(super) fn severity_chips<'a>(
    counts: [(Severity, usize); 4],
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let style = ChipStyle {
        fill: r.field,
        edge: theme_ext::border_color(tokens),
        hover: r.field,
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    let metrics = ChipMetrics::from_text_size(TS.body);
    let mut chips: Row<Element<'a, Message>> = Row::new()
        .spacing(metrics.row_gap)
        .align_y(Alignment::Center);
    for (severity, count) in counts {
        let (caption, tag, tone) = chip_look(severity);
        let tag: Label<'a> = match tag {
            Some(letter) => t!(i18n, letter).into(),
            // raw: a dash for Off, the same in every language.
            None => raw("\u{2013}".to_owned()),
        };
        let (fill, ink) = pill_colors(tokens, tone);
        chips = chips.push(tag_count(
            tag,
            ChipTone { fill, ink },
            t!(i18n, caption),
            u32::try_from(count).unwrap_or(u32::MAX),
            style,
            metrics,
            i18n,
        ));
    }
    chips.boxed()
}

impl Signex {
    /// The strip under the title: source pill, scope note (with the
    /// machine-rules notice when it applies) and the severity chips, over a
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
        if !stores_rules && let Some(issue) = self.machine_rules_issue() {
            note = note.push(
                text(issue.localized(i18n))
                    .size(TS.caption)
                    .color(tokens::danger(theme_tokens))
                    .boxed(),
            );
        }

        let chips = severity_chips(self.erc_dialog_severity_counts(), theme_tokens, i18n);

        let line: Row<Element<'a, Message>> = Row::new()
            .push(source.boxed())
            .push(note.boxed())
            .push(chips)
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
    fn chips_tone_only_the_reporting_severities() {
        assert_eq!(chip_look(Severity::Error).2, PillTone::Err);
        assert_eq!(chip_look(Severity::Warning).2, PillTone::Warn);
        assert_eq!(chip_look(Severity::Info).2, PillTone::Info);
        assert_eq!(chip_look(Severity::Off).2, PillTone::Neutral);
        assert_eq!(chip_look(Severity::Off).0, ids::DIALOG_ERC_CHIP_OFF);
        assert_eq!(
            chip_look(Severity::Error).1,
            Some(ids::DIALOG_RULES_ERROR_SHORT)
        );
        assert_eq!(chip_look(Severity::Off).1, None);
    }

    /// Mockup row 84: the counts are compact one-line chips, "E Errors 15",
    /// the tag drawn smaller than its caption.
    #[test]
    fn the_counts_draw_as_one_line_tagged_chips() {
        let (app, _boot) = crate::app::Signex::new();
        let tokens = app.document_state.panel_ctx.tokens.clone();
        let i18n = I18n::en_us();
        let counts = [
            (Severity::Error, 15),
            (Severity::Warning, 12),
            (Severity::Info, 3),
            (Severity::Off, 3),
        ];
        let texts = crate::layout_probe::lay_out_texts(
            severity_chips(counts, &tokens, &i18n),
            iced::Size::new(1000.0, 200.0),
        );
        let words: Vec<String> = texts
            .iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        assert_eq!(
            words,
            [
                "E", "Errors", "15", "W", "Warnings", "12", "I", "Info", "3", "\u{2013}", "Off",
                "3"
            ],
            "{texts:#?}"
        );
        let centre = |b: &iced::Rectangle| b.y + b.height / 2.0;
        let line = centre(&texts[0].bounds);
        for t in &texts {
            assert!((centre(&t.bounds) - line).abs() < 2.0, "{texts:#?}");
        }
        assert!(
            texts[0].bounds.height < texts[1].bounds.height,
            "{texts:#?}"
        );
    }
}
