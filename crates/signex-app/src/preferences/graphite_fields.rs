//! Graphite controls of the Preferences dialog: the nav search field, the
//! mock's page names and the font pick-list.
//! Legacy themes never reach these helpers.

use super::{PrefMsg, PrefNav, SP, TS};
use crate::styles::{self, ti, tokens};
use iced::widget::{container, pick_list, scrollable, text_input};
use iced::{Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;

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

/// The UI-font pick-list drawn as a Graphite field (field fill, hairline,
/// control radius; the menu is a raised card).
pub(super) fn font_pick_list<'a>(
    current: String,
    families: &'a [String],
    _t: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    pick_list(Some(current), families, |s: &String| s.clone())
        .graphite()
        .on_select(PrefMsg::DraftFont)
        .padding([SP.s, SP.m])
        .width(200)
        .boxed()
}

/// `scrollable` style of the Preferences panes: a thin neutral scroller.
pub(super) fn scroll_style(
    t: &ThemeTokens,
) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style + 'static {
    let r = tokens::resolve(t);
    let (thumb, thumb_hot) = (r.line_strong, r.ink4);
    move |theme: &Theme, status: scrollable::Status| {
        let mut style = scrollable::default(theme, status);
        let idle = matches!(status, scrollable::Status::Active { .. });
        for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
            rail.background = None;
            rail.border = Border::default();
            rail.scroller.background = Background::Color(if idle { thumb } else { thumb_hot });
            rail.scroller.border = Border {
                radius: 999.0.into(),
                ..Border::default()
            };
        }
        style
    }
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
