//! Graphite controls of the Preferences dialog: the nav search field, the
//! mock's page names and the font pick-list.
//! Legacy themes never reach these helpers.

use super::{PrefMsg, PrefNav, SP, TS};
use crate::styles::{self, ti, tokens};
use iced::widget::{container, pick_list, scrollable};
use iced::{Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::text_input;

/// The page name on Graphite: the mock's wording for the pages it names.
pub(super) fn graphite_label(item: PrefNav) -> MessageId {
    match item {
        PrefNav::Appearance => ids::PREFERENCES_PAGE_THEME,
        PrefNav::Keyboard => ids::PREFERENCES_PAGE_KEYBOARD,
        PrefNav::Editing => ids::PREFERENCES_PAGE_EDITING,
        PrefNav::Annotation => ids::PREFERENCES_PAGE_ANNOTATION,
        PrefNav::RuleProfiles => ids::PREFERENCES_PAGE_RULE_PROFILES,
        PrefNav::LibraryDistributors => ids::PREFERENCES_PAGE_DISTRIBUTORS,
        PrefNav::ComponentClasses => ids::PREFERENCES_PAGE_COMPONENT_CLASSES,
    }
}

/// The page's group: its name in the dialog subtitle and its nav heading
/// (written in capitals).
pub(super) fn group_messages(item: PrefNav) -> (MessageId, MessageId) {
    match item.group() {
        "Schematic" => (
            ids::PREFERENCES_GROUP_SCHEMATIC,
            ids::PREFERENCES_GROUP_SCHEMATIC_HEADING,
        ),
        "Validation" => (
            ids::PREFERENCES_GROUP_VALIDATION,
            ids::PREFERENCES_GROUP_VALIDATION_HEADING,
        ),
        "Library" => (
            ids::PREFERENCES_GROUP_LIBRARY,
            ids::PREFERENCES_GROUP_LIBRARY_HEADING,
        ),
        _ => (
            ids::PREFERENCES_GROUP_SYSTEM,
            ids::PREFERENCES_GROUP_SYSTEM_HEADING,
        ),
    }
}

/// Whether a nav row stays visible for the search `query`: the casemap fold
/// of the UI language over the shown page name and group, and the English
/// page and group names. An empty query shows every row.
pub(super) fn nav_matches(i18n: &I18n, item: PrefNav, query: &str) -> bool {
    let q = i18n.fold_for_search(query.trim());
    let page = t!(i18n, graphite_label(item)).into_string();
    let group = t!(i18n, group_messages(item).0).into_string();
    q.is_empty()
        || [page.as_str(), group.as_str(), item.label(), item.group()]
            .into_iter()
            .any(|name| i18n.fold_for_search(name).contains(&q))
}

/// The "Search preferences" field above the nav: a bordered frame around a
/// transparent input.
pub(super) fn nav_search<'a>(query: &'a str, t: &ThemeTokens, i18n: &I18n) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(t);
    let (ink4, ink) = (r.ink4, ti(t.text));
    let input = text_input(t!(i18n, ids::PREFERENCES_SEARCH), query)
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
        let en = I18n::en_us();
        assert!(PrefNav::ALL.iter().all(|&n| nav_matches(&en, n, "  ")));
    }

    #[test]
    fn search_matches_page_name_and_group_case_insensitively() {
        let en = I18n::en_us();
        assert!(nav_matches(&en, PrefNav::Appearance, "THEME"));
        assert!(nav_matches(&en, PrefNav::Appearance, "appear"));
        assert!(nav_matches(&en, PrefNav::Annotation, "schem"));
        assert!(!nav_matches(&en, PrefNav::Annotation, "theme"));
    }

    #[test]
    fn every_group_has_its_own_messages() {
        let en = I18n::en_us();
        for &item in PrefNav::ALL {
            let (name, heading) = group_messages(item);
            assert_eq!(t!(en, name).into_string(), item.group(), "{item:?}");
            assert_eq!(
                t!(en, heading).into_string(),
                en.upper_for_display(item.group()),
                "{item:?}"
            );
        }
    }
}
