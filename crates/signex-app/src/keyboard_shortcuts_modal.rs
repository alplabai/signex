//! Help ▸ Keyboard Shortcuts modal — single-page reference grouped by
//! category. Reachable from the Help menu and from F1 (which also closes
//! the modal when it's already open).
//!
//! Closes UX_IMPROVEMENTS_OVER_ALTIUM §4.2 ("Hotkey discoverability").

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Element, Length, Widget};
use signex_i18n::I18n;
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use crate::app::view::dialogs::modal_title_header;
use crate::app::{Message, OverlayMsg};
use crate::keymap::{KeymapEditorModel, KeymapEditorRow, ShortcutProfileSet};
use crate::styles;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const MODAL_W: f32 = 520.0;
const MODAL_H: f32 = 600.0;
const KEY_COL_W: f32 = 140.0;
const SECTION_HEADER_GAP_TOP: f32 = 14.0;

/// One category of the reference sheet: the category id and the
/// `(label, trigger)` rows listed under it, in profile order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutSection {
    pub category: String,
    pub rows: Vec<(String, String)>,
}

/// Group the active profile's listed shortcuts by category, categories in
/// first-seen order. Pure: the result depends only on `profiles`, so
/// `UiState` caches it and rebuilds on a keymap change instead of the view
/// rebuilding it every frame (#1380).
pub fn build_sections(profiles: &ShortcutProfileSet) -> Vec<ShortcutSection> {
    let editor = KeymapEditorModel::new(profiles.clone());
    let mut sections: Vec<ShortcutSection> = Vec::new();
    let mut index = std::collections::HashMap::<String, usize>::new();
    for row in editor.rows() {
        let slot = *index.entry(row.category.clone()).or_insert_with(|| {
            sections.push(ShortcutSection {
                category: row.category.clone(),
                rows: Vec::new(),
            });
            sections.len() - 1
        });
        if !row.trigger.trim().is_empty() && is_listed(&row) {
            sections[slot].rows.push((row.label, row.trigger));
        }
    }
    sections.retain(|section| !section.rows.is_empty());
    sections
}

/// The sheet painted as a card in the main window.
pub fn view<'a>(
    i18n: &I18n,
    tokens: &'a ThemeTokens,
    sections: &[ShortcutSection],
) -> Element<'a, Message> {
    build(i18n, tokens, sections, false)
}

/// The sheet as the content of its own OS window: no header, filling it.
pub fn window_view<'a>(
    i18n: &I18n,
    tokens: &'a ThemeTokens,
    sections: &[ShortcutSection],
) -> Element<'a, Message> {
    build(i18n, tokens, sections, true)
}

fn build<'a>(
    i18n: &I18n,
    tokens: &'a ThemeTokens,
    sections: &[ShortcutSection],
    windowed: bool,
) -> Element<'a, Message> {
    let text_primary = styles::ti(tokens.text);
    let text_secondary = styles::ti(tokens.text_secondary);
    let border = styles::ti(tokens.border);
    // Row dividers should be barely visible — half the alpha of the
    // border colour. Subtle horizontal rhythm without noise.
    let row_divider_color = iced::Color {
        a: border.a * 0.35,
        ..border
    };
    let section_divider_color = iced::Color {
        a: border.a * 0.7,
        ..border
    };

    // ── Header — canonical modal chrome ─────────────────────────
    let header = modal_title_header(
        "Keyboard Shortcuts",
        Message::Overlay(OverlayMsg::CloseKeyboardShortcuts),
        tokens,
    );

    // ── Body — sections per category ────────────────────────────
    // Column header (Action | Shortcut). Action goes on the LEFT
    // because it's what users scan for; Shortcut on the right.
    let column_header = container(
        row![
            text(raw("Action")).size(TS.caption).color(text_secondary),
            Space::new().width(Length::Fill),
            text(raw("Shortcut"))
                .size(TS.caption)
                .color(text_secondary)
                .width(Length::Fixed(KEY_COL_W)),
        ]
        .padding([0, 0]),
    )
    .padding([6, 0]);

    let mut body: Column<Element<'a, Message>> =
        Column::new().spacing(0).push(column_header.boxed());
    body = body.push(crate::panels::thin_sep(border));

    let mut first_section = true;
    for section in sections {
        let category = &section.category;
        let section_shortcuts = &section.rows;

        // Subtle section divider above every section after the first.
        if !first_section {
            body = body.push(crate::panels::thin_sep(section_divider_color));
        }
        first_section = false;

        body = body.push(
            container(
                text(raw(i18n.upper_for_display(&title_case(i18n, category))))
                    .size(TS.caption)
                    .color(text_secondary),
            )
            .padding(iced::Padding {
                top: SECTION_HEADER_GAP_TOP,
                right: 0.0,
                bottom: 4.0,
                left: 0.0,
            })
            .boxed(),
        );

        let last_idx = section_shortcuts.len().saturating_sub(1);
        let mut section_col: Column<Element<'a, Message>> = Column::new().spacing(0);
        for (idx, (label, trigger)) in section_shortcuts.iter().enumerate() {
            let row_el: Element<'a, Message> = row![
                text(raw(label.clone())).size(TS.body).color(text_primary),
                Space::new().width(Length::Fill),
                text(raw(trigger.clone()))
                    .size(TS.body)
                    .color(text_primary)
                    .width(Length::Fixed(KEY_COL_W)),
            ]
            .padding([3, 0])
            .boxed();
            section_col = section_col.push(row_el);
            // Subtle row divider between rows (not after the last row
            // in a section — section divider handles that gap).
            if idx < last_idx {
                section_col = section_col.push(crate::panels::thin_sep(row_divider_color));
            }
        }
        body = body.push(section_col.boxed());
    }

    let body_container = container(scrollable(body).height(Length::Fill))
        .padding([10.0, SP.xl])
        .width(Length::Fill)
        .height(Length::Fill);

    // ── Card ────────────────────────────────────────────────────
    if windowed {
        return container(body_container)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::modal_card(tokens))
            .boxed();
    }
    let card = container(column![header, body_container].spacing(0))
        .width(Length::Fixed(MODAL_W))
        .height(Length::Fixed(MODAL_H))
        .clip(true)
        .style(styles::modal_card(tokens));

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}

/// Whether the reference sheet may list `row` as a working shortcut.
///
/// A catalog command with no dispatch arm consumes its key and does
/// nothing (#103), so listing it would advertise a dead binding. Rows
/// without a command id are kept as they were.
fn is_listed(row: &KeymapEditorRow) -> bool {
    !row.unbound
        && row
            .command
            .as_ref()
            .is_none_or(crate::app::command::bridge::is_dispatchable)
}

use crate::keymap::category_title_case as title_case;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::KeymapEditorModel;

    #[test]
    fn category_titles_capitalise_by_the_message_language() {
        let tr = I18n::with_message_locale_for_test("tr");
        assert_eq!(title_case(&tr, "idle_tools"), "İdle Tools");
        assert_eq!(title_case(&I18n::en_us(), "idle_tools"), "Idle Tools");
        assert_eq!(tr.upper_for_display("ilk"), "İLK");
    }

    /// The pre-cache algorithm (#1380), kept as the oracle.
    fn legacy_sections(profiles: &ShortcutProfileSet) -> Vec<(String, Vec<(String, String)>)> {
        let rows = KeymapEditorModel::new(profiles.clone()).rows();
        let mut categories = Vec::<String>::new();
        for row in &rows {
            if !categories.iter().any(|category| category == &row.category) {
                categories.push(row.category.clone());
            }
        }
        categories
            .into_iter()
            .map(|category| {
                let listed = rows
                    .iter()
                    .filter(|row| {
                        row.category == category && !row.trigger.trim().is_empty() && is_listed(row)
                    })
                    .map(|row| (row.label.clone(), row.trigger.clone()))
                    .collect::<Vec<_>>();
                (category, listed)
            })
            .filter(|(_, listed)| !listed.is_empty())
            .collect()
    }

    #[test]
    fn build_sections_equals_the_per_frame_algorithm_for_every_profile() {
        let mut set = ShortcutProfileSet::built_ins().expect("built-ins load");
        let ids: Vec<String> = set.profiles().map(|p| p.id.clone()).collect();
        for id in ids {
            set.set_active_profile(id).unwrap();
            let built: Vec<_> = build_sections(&set)
                .into_iter()
                .map(|s| (s.category, s.rows))
                .collect();
            assert_eq!(built, legacy_sections(&set));
            assert!(!built.is_empty());
        }
    }

    /// #103 - the sheet never lists a binding whose command has no
    /// dispatch arm, and still lists the ones that work.
    #[test]
    fn the_sheet_lists_only_shortcuts_that_dispatch() {
        let rows = KeymapEditorModel::built_ins()
            .expect("built-in profiles load")
            .rows();
        let with_trigger = |row: &&KeymapEditorRow| !row.trigger.trim().is_empty();

        let listed: Vec<_> = rows
            .iter()
            .filter(with_trigger)
            .filter(|r| is_listed(r))
            .collect();
        let hidden: Vec<_> = rows
            .iter()
            .filter(with_trigger)
            .filter(|r| !is_listed(r))
            .collect();

        assert!(
            listed
                .iter()
                .any(|r| r.label.to_lowercase().contains("undo")),
            "a working shortcut must stay listed"
        );
        assert!(
            hidden.iter().any(|r| r
                .command
                .as_ref()
                .is_some_and(|c| c.as_str() == "cycle_wiring_mode")),
            "an unimplemented command's binding must be hidden"
        );
        assert!(listed.iter().all(|r| {
            r.command
                .as_ref()
                .is_none_or(crate::app::command::bridge::is_dispatchable)
        }));
    }
}
