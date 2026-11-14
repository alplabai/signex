//! The picker modal's cards for the placed-part stages: the pin preview of
//! Change Component (#1260) and the datasheet list (#1301). Pure: the lines
//! shown are built by [`preview_lines`] from the engine's preview.

use iced::widget::{Space, column, container, scrollable};
use iced::{Element, Length, Widget};
use signex_engine::ComponentChangePreview;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::schematic::PinMatchedBy;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{button, raw, text};

use crate::library::messages::{LibraryMessage, PlacedPartMsg};
use crate::library::modal_chrome::{cancel_btn, footer_strip, header, primary_btn, secondary_btn};
use crate::library::state::{ChangePreview, DatasheetChoice};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
const CARD_W: f32 = 520.0;
const CARD_H: f32 = 440.0;

/// How a preview line reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Info,
    /// A connection that would be lost, moved or merged.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewLine {
    pub kind: LineKind,
    pub text: String,
}

/// The lines of the pin preview: the warnings first, then the mapping.
pub fn preview_lines(preview: &ComponentChangePreview) -> Vec<PreviewLine> {
    let warn = |text: String| PreviewLine {
        kind: LineKind::Warning,
        text,
    };
    let list = |keys: &[String]| keys.join(", ");
    let mut lines = Vec::new();
    if let Some(unit) = preview.missing_unit {
        lines.push(warn(format!(
            "The new part has no drawing for unit {unit}."
        )));
    }
    if !preview.unmatched_connected.is_empty() {
        lines.push(warn(format!(
            "Connected pins with no match, their wires will dangle: {}",
            list(&preview.unmatched_connected)
        )));
    }
    if !preview.moved_connected.is_empty() {
        lines.push(warn(format!(
            "Connected pins that sit elsewhere on the new part, their wires will no longer reach them: {}",
            list(&preview.moved_connected)
        )));
    }
    if !preview.gained.is_empty() {
        lines.push(warn(format!(
            "New pins landing on a wire they were not mapped to: {}",
            list(&preview.gained)
        )));
    }
    for lost in &preview.changed_data {
        let new = if lost.new.is_empty() {
            "cleared"
        } else {
            lost.new.as_str()
        };
        lines.push(warn(format!("{} {} \u{2192} {new}", lost.label, lost.old)));
    }
    if preview.is_clean() {
        lines.push(PreviewLine {
            kind: LineKind::Info,
            text: "Every connected pin keeps its wire.".into(),
        });
    }
    if !preview.replaced.is_empty() {
        lines.push(PreviewLine {
            kind: LineKind::Info,
            text: format!("Replaced by the new part: {}", list(&preview.replaced)),
        });
    }
    if !preview.kept.is_empty() {
        lines.push(PreviewLine {
            kind: LineKind::Info,
            text: format!("Kept: {}", list(&preview.kept)),
        });
    }
    for pair in &preview.mapping.pairs {
        let how = match pair.by {
            PinMatchedBy::Number => "",
            PinMatchedBy::Name => " (by name)",
        };
        lines.push(PreviewLine {
            kind: LineKind::Info,
            text: format!("Pin {} \u{2192} {}{how}", pair.old, pair.new),
        });
    }
    for old in &preview.mapping.unmatched {
        lines.push(PreviewLine {
            kind: LineKind::Info,
            text: format!("Pin {old} has no match"),
        });
    }
    lines
}

/// The confirm button's label: it says so when the change loses something.
pub fn confirm_label(preview: &ComponentChangePreview) -> MessageId {
    if preview.is_clean() {
        ids::LIBRARY_CHANGE_PART_CONFIRM
    } else {
        ids::LIBRARY_CHANGE_PART_CONFIRM_ANYWAY
    }
}

fn card<'a>(
    title: &str,
    body: Element<'a, LibraryMessage>,
    buttons: Vec<Element<'a, LibraryMessage>>,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let header = header(title, LibraryMessage::ClosePicker, windowed, tokens);
    let footer = footer_strip(Vec::new(), buttons, tokens);
    container(
        column![
            header,
            container(body).padding(SP.l).height(Length::Fill),
            footer
        ]
        .width(crate::library::card_len(windowed, CARD_W))
        .height(crate::library::card_len(windowed, CARD_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// One full-width option of the datasheet list.
fn option_button<'a>(
    label: &str,
    tokens: &ThemeTokens,
    message: LibraryMessage,
) -> Element<'a, LibraryMessage> {
    button(
        text(raw(label.to_string()))
            .size(TS.body)
            .color(theme_ext::text_primary(tokens)),
    )
    .padding([SP.s, SP.l])
    .width(Length::Fill)
    .on_press(message)
    .style(crate::styles::outline_button(tokens))
    .boxed()
}

/// Step two of Change Component: what the replacement does to the pins.
pub fn change_preview_view<'a>(
    change: &'a ChangePreview,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let warning = theme_ext::warning_color(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let mut lines = column![
        text(raw(format!(
            "{} becomes {}. Its designator and parameters stay.",
            change.reference, change.resolved.internal_pn
        )))
        .size(TS.body)
        .color(text_c),
        Space::new().height(SP.m),
    ]
    .spacing(SP.s);
    for line in preview_lines(&change.preview) {
        let color = match line.kind {
            LineKind::Info => muted,
            LineKind::Warning => warning,
        };
        lines = lines.push(text(raw(line.text)).size(TS.body).color(color).boxed());
    }
    let buttons = vec![
        secondary_btn(
            t!(i18n, ids::DIALOG_BUTTON_BACK),
            LibraryMessage::PlacedPart(PlacedPartMsg::ChangePreviewBackedOut),
            tokens,
        ),
        cancel_btn(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            LibraryMessage::ClosePicker,
            tokens,
        ),
        primary_btn(
            t!(i18n, confirm_label(&change.preview)),
            LibraryMessage::PlacedPart(PlacedPartMsg::ChangeConfirmed),
            tokens,
        ),
    ];
    card(
        crate::library::picker::TITLE_CHANGE_COMPONENT,
        scrollable(lines).width(Length::Fill).boxed(),
        buttons,
        tokens,
        windowed,
    )
}

/// The documents of a part with more than one: pick which to open.
pub fn datasheet_choice_view<'a>(
    choice: &'a DatasheetChoice,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let mut list = column![
        text(raw(format!(
            "Which document of {} do you want to open?",
            choice.reference
        )))
        .size(TS.body)
        .color(theme_ext::text_primary(tokens)),
        Space::new().height(SP.m),
    ]
    .spacing(SP.s);
    for (index, target) in choice.targets.iter().enumerate() {
        list = list.push(option_button(
            &target.label,
            tokens,
            LibraryMessage::PlacedPart(PlacedPartMsg::DatasheetPicked(index)),
        ));
    }
    let buttons = vec![cancel_btn(
        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
        LibraryMessage::ClosePicker,
        tokens,
    )];
    card(
        crate::library::picker::TITLE_OPEN_DATASHEET,
        scrollable(list).width(Length::Fill).boxed(),
        buttons,
        tokens,
        windowed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{PinMapping, PinPair};

    fn english(id: MessageId) -> String {
        I18n::en_us_ref().message(id, &[]).to_string()
    }

    fn pair(old: &str, new: &str, by: PinMatchedBy) -> PinPair {
        PinPair {
            old: old.into(),
            new: new.into(),
            by,
        }
    }

    #[test]
    fn a_clean_preview_says_every_wire_stays_and_lists_the_mapping() {
        let preview = ComponentChangePreview {
            mapping: PinMapping {
                pairs: vec![
                    pair("1", "1", PinMatchedBy::Number),
                    pair("3", "9", PinMatchedBy::Name),
                ],
                unmatched: vec![],
            },
            ..Default::default()
        };
        let lines: Vec<String> = preview_lines(&preview)
            .into_iter()
            .map(|l| l.text)
            .collect();
        assert_eq!(
            lines,
            [
                "Every connected pin keeps its wire.",
                "Pin 1 \u{2192} 1",
                "Pin 3 \u{2192} 9 (by name)"
            ]
        );
        assert_eq!(english(confirm_label(&preview)), "Change");
    }

    #[test]
    fn part_data_that_is_cleared_or_changed_is_warned_about_with_its_old_value() {
        let preview = ComponentChangePreview {
            changed_data: vec![
                signex_engine::DataChange {
                    label: "Footprint".into(),
                    old: "R_0603".into(),
                    new: String::new(),
                },
                signex_engine::DataChange {
                    label: "MPN".into(),
                    old: "OLD".into(),
                    new: "NEW".into(),
                },
            ],
            ..Default::default()
        };
        let warnings: Vec<String> = preview_lines(&preview)
            .into_iter()
            .filter(|l| l.kind == LineKind::Warning)
            .map(|l| l.text)
            .collect();
        assert_eq!(
            warnings,
            ["Footprint R_0603 \u{2192} cleared", "MPN OLD \u{2192} NEW"]
        );
    }

    #[test]
    fn lost_connections_are_warned_about_first_and_the_confirm_says_so() {
        let preview = ComponentChangePreview {
            mapping: PinMapping {
                pairs: vec![pair("1", "1", PinMatchedBy::Number)],
                unmatched: vec!["3".into()],
            },
            unmatched_connected: vec!["3".into()],
            moved_connected: vec!["2".into()],
            gained: vec!["7".into()],
            missing_unit: Some(2),
            replaced: vec!["Footprint".into(), "MPN".into()],
            kept: vec!["Designator".into(), "Tolerance".into()],
            changed_data: Vec::new(),
        };
        let lines = preview_lines(&preview);
        let warnings: Vec<&str> = lines
            .iter()
            .filter(|l| l.kind == LineKind::Warning)
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(warnings.len(), 4);
        assert!(warnings[0].contains("unit 2"));
        assert!(warnings[1].ends_with(": 3"));
        assert!(warnings[2].ends_with(": 2"));
        assert!(warnings[3].ends_with(": 7"));
        assert!(lines[..4].iter().all(|l| l.kind == LineKind::Warning));
        assert!(lines.iter().any(|l| l.text == "Pin 3 has no match"));
        assert_eq!(english(confirm_label(&preview)), "Change anyway");
        assert!(
            lines
                .iter()
                .any(|l| l.text == "Replaced by the new part: Footprint, MPN")
        );
        assert!(
            lines
                .iter()
                .any(|l| l.text == "Kept: Designator, Tolerance")
        );
    }
}
