//! Graphite controls of the Preferences dialog: the nav search field, the
//! mock's page names and the pane scroller style.
//! Legacy themes never reach these helpers.

use super::{PrefMsg, PrefNav, SP, TS};
use crate::styles::{self, ti, tokens};
use iced::widget::{container, scrollable};
use iced::{Background, Border, Color, Element, Length, Padding, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text_input;

/// The page name on Graphite: the mock's wording for the pages it names.
pub(super) fn graphite_label(item: PrefNav) -> MessageId {
    match item {
        PrefNav::Appearance => ids::PREFERENCES_PAGE_THEME,
        PrefNav::Language => ids::PREFERENCES_PAGE_LANGUAGE,
        PrefNav::Keyboard => ids::PREFERENCES_PAGE_KEYBOARD,
        PrefNav::General => ids::PREFERENCES_PAGE_GENERAL,
        PrefNav::Editing => ids::PREFERENCES_PAGE_EDITING,
        PrefNav::SchematicDisplay => ids::PREFERENCES_PAGE_SCHEMATIC_DISPLAY,
        PrefNav::PcbDefaults => ids::PREFERENCES_PAGE_PCB_DEFAULTS,
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
        "PCB" => (
            ids::PREFERENCES_GROUP_PCB,
            ids::PREFERENCES_GROUP_PCB_HEADING,
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

/// The English names a page went by before the mockup row 143 regroup, so a search
/// for them still finds it.
fn former_names(item: PrefNav) -> &'static [&'static str] {
    match item {
        PrefNav::Appearance => &["Theme"],
        PrefNav::Editing => &["Graphical editing"],
        PrefNav::ComponentClasses => &["Default Component Classes"],
        PrefNav::Language
        | PrefNav::Keyboard
        | PrefNav::General
        | PrefNav::Annotation
        | PrefNav::SchematicDisplay
        | PrefNav::PcbDefaults
        | PrefNav::RuleProfiles
        | PrefNav::LibraryDistributors => &[],
    }
}

/// The labels of the setting rows a page draws, so a search for a setting
/// finds its page (mockup row 143). A row added to a page goes here too;
/// a layout test in `app::view::chrome` checks each page draws its list.
/// The table pages (shortcuts, profiles, distributors, classes) have none.
pub(crate) fn setting_labels(item: PrefNav) -> &'static [MessageId] {
    match item {
        PrefNav::Appearance => &[
            ids::PREFERENCES_APPEARANCE_UI_FONT,
            ids::PREFERENCES_APPEARANCE_UI_SCALE,
            ids::PREFERENCES_SELECTED_SEGMENT_LABEL,
            ids::PREFERENCES_APPEARANCE_GRID_STYLE_LABEL,
            ids::PREFERENCES_APPEARANCE_SYMBOL_GRID_LABEL,
            ids::PREFERENCES_APPEARANCE_PIN_SELECTION_LABEL,
            ids::PREFERENCES_APPEARANCE_GPU_RENDER_LABEL,
            ids::PREFERENCES_APPEARANCE_ANTIALIASING,
            ids::PREFERENCES_WHEEL_MODE_LABEL,
            ids::PREFERENCES_APPEARANCE_ZOOM_SPEED,
            ids::PREFERENCES_APPEARANCE_TRACKPAD_PAN,
            ids::PREFERENCES_LAYER_SWITCHER_LABEL,
            ids::PREFERENCES_APPEARANCE_WINDOW_BUTTONS,
            ids::PREFERENCES_APPEARANCE_REMEMBER_GEOMETRY,
        ],
        PrefNav::Language => &[ids::PREFERENCES_LANGUAGE_INTERFACE],
        PrefNav::General => &[
            ids::PREFERENCES_AUTOSAVE,
            ids::PREFERENCES_AGENT_CONNECTIONS_LABEL,
        ],
        PrefNav::Annotation => &[ids::PREFERENCES_ANNOTATE_UNDO],
        PrefNav::Editing => &[
            ids::PREFERENCES_MOVE_KEEPS_WIRES,
            ids::PREFERENCES_DRAG_DIRECTION,
            ids::PREFERENCES_NET_HOVER,
            ids::PREFERENCES_APPEARANCE_FAST_GRID_1,
            ids::PREFERENCES_APPEARANCE_FAST_GRID_2,
            ids::PREFERENCES_INCREMENT_STEP,
            ids::PREFERENCES_PASTE_DESIGNATORS,
        ],
        PrefNav::SchematicDisplay => &[
            ids::PREFERENCES_SYMBOL_BODY_FILL,
            ids::PREFERENCES_SYMBOL_BODY_FILL_COLOUR,
        ],
        PrefNav::PcbDefaults => &[
            ids::PCB_PRIM_LINE_WIDTH,
            ids::PCB_PRIM_TEXT_HEIGHT,
            ids::PCB_PRIM_PRECISION,
            ids::PCB_PRIM_DIM_ARROW_STYLE,
            ids::PCB_PRIM_DIM_ARROW_LENGTH,
            ids::PCB_PRIM_DIM_ARROW_WIDTH,
            ids::PCB_PRIM_DIM_FONT,
            ids::PCB_PRIM_DIM_BOLD,
            ids::PCB_PRIM_DIM_ITALIC,
            ids::PCB_PRIM_DIM_TEXT_WIDTH,
            ids::PCB_PRIM_DIM_TEXT_POSITION,
            ids::PCB_PRIM_DIM_TEXT_GAP,
            ids::PCB_PRIM_DIM_EXT_OFFSET,
            ids::PCB_PRIM_DIM_EXT_OVERSHOOT,
            ids::PCB_PRIM_DIM_EXT_WIDTH,
            ids::PCB_PRIM_DIM_UNIT,
            ids::PCB_PRIM_DIM_SHOW_UNIT,
            ids::PCB_PRIM_DIM_PREFIX,
            ids::PCB_PRIM_DIM_SUFFIX,
            ids::PCB_PRIM_DIM_TRAILING_ZEROS,
        ],
        PrefNav::Keyboard
        | PrefNav::RuleProfiles
        | PrefNav::LibraryDistributors
        | PrefNav::ComponentClasses => &[],
    }
}

/// Whether a nav row stays visible for the search `query`: the casemap fold
/// of the UI language over the shown page name and group, the English page
/// and group names, the page's former English names and its setting labels
/// in the UI language and in English, as the page names are. An empty query
/// shows every row.
pub(super) fn nav_matches(i18n: &I18n, item: PrefNav, query: &str) -> bool {
    let q = i18n.fold_for_search(query.trim());
    if q.is_empty() {
        return true;
    }
    let page = t!(i18n, graphite_label(item)).into_string();
    let group = t!(i18n, group_messages(item).0).into_string();
    let en = I18n::en_us();
    let settings: Vec<String> = setting_labels(item)
        .iter()
        .flat_map(|&id| [t!(i18n, id).into_string(), t!(en, id).into_string()])
        .collect();
    [page.as_str(), group.as_str(), item.label(), item.group()]
        .into_iter()
        .chain(former_names(item).iter().copied())
        .chain(settings.iter().map(String::as_str))
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

    /// Mockup row 143: the regrouped pages keep answering to their former names, and
    /// the mock's "an" search finds the new Language page.
    #[test]
    fn search_finds_the_former_page_names_and_language() {
        let en = I18n::en_us();
        assert!(nav_matches(&en, PrefNav::Editing, "graphical"));
        assert!(nav_matches(
            &en,
            PrefNav::ComponentClasses,
            "default component"
        ));
        assert!(nav_matches(&en, PrefNav::Language, "an"));
        assert!(!nav_matches(&en, PrefNav::Editing, "theme"));
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
