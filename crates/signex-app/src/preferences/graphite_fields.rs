//! Graphite controls of the Preferences dialog: the nav search field, the
//! mock's page names, the font pick-list and the ghost theme buttons.
//! Legacy themes never reach these helpers.

use super::{PrefMsg, PrefNav, SP, TS};
use crate::styles::{self, DialogButtonKind, Radii, dialog_button, ti, tokens};
use iced::overlay::menu;
use iced::widget::{container, pick_list, text_input};
use iced::{Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

/// The page name on Graphite: the mock's wording for the pages it names.
pub(super) fn graphite_label(item: PrefNav) -> &'static str {
    match item {
        PrefNav::Appearance => "Theme",
        PrefNav::Keyboard => "Keyboard shortcuts",
        PrefNav::Editing => "Graphical editing",
        other => other.label(),
    }
}

/// Whether a nav row stays visible for the search `query` (case-insensitive,
/// over the page name and its group). An empty query shows every row.
pub(super) fn nav_matches(item: PrefNav, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty()
        || graphite_label(item).to_lowercase().contains(&q)
        || item.label().to_lowercase().contains(&q)
        || item.group().to_lowercase().contains(&q)
}

/// The "Search preferences" field above the nav: a bordered frame around a
/// transparent input.
pub(super) fn nav_search<'a>(query: &'a str, t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (ink4, ink) = (r.ink4, ti(t.text));
    let input = text_input("Search preferences", query)
        .on_input(PrefMsg::NavSearchChanged)
        .padding(Padding::ZERO)
        .size(TS.label)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..ink },
        });
    container(input)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(styles::field(t, false))
        .boxed()
}

/// A ghost button for the Import / Export theme actions.
pub(super) fn theme_io_button<'a>(
    label: &str,
    msg: PrefMsg,
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    dialog_button(label, DialogButtonKind::Ghost, Some(msg), t)
}

/// The UI-font pick-list drawn as a Graphite field (field fill, hairline,
/// control radius; the menu is a raised card).
pub(super) fn font_pick_list<'a>(
    current: String,
    families: &'a [String],
    t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (field, ink, ink3) = (r.field, ti(t.text), r.ink3);
    let (line, accent, raised) = (ti(t.border), ti(t.accent), r.raised);
    let (selected, radius) = (ti(t.selection), Radii::GRAPHITE.control);
    pick_list(Some(current), families, |s: &String| s.clone())
        .on_select(PrefMsg::DraftFont)
        .text_size(TS.label)
        .padding([SP.s, SP.m])
        .width(200)
        .style(move |_: &Theme, status| pick_list::Style {
            text_color: ink,
            placeholder_color: ink3,
            handle_color: ink3,
            background: Background::Color(field),
            border: Border {
                width: 1.0,
                color: if matches!(status, pick_list::Status::Opened { .. }) {
                    accent
                } else {
                    line
                },
                radius: radius.into(),
                ..Border::default()
            },
        })
        .menu_style(move |_: &Theme| menu::Style {
            background: Background::Color(raised),
            border: Border {
                width: 1.0,
                color: line,
                radius: radius.into(),
                ..Border::default()
            },
            text_color: ink,
            selected_text_color: ink,
            selected_background: Background::Color(selected),
            shadow: Default::default(),
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_search_keeps_every_page() {
        assert!(PrefNav::ALL.iter().all(|&n| nav_matches(n, "  ")));
    }

    #[test]
    fn search_matches_page_name_and_group_case_insensitively() {
        assert!(nav_matches(PrefNav::Appearance, "THEME"));
        assert!(nav_matches(PrefNav::Appearance, "appear"));
        assert!(nav_matches(PrefNav::Annotation, "schem"));
        assert!(!nav_matches(PrefNav::Annotation, "theme"));
    }
}
