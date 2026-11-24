//! #1769 - the sidebar shows each group heading once; the nav order and
//! nav search (mockup rows 143 and 147).

use super::PrefNav;

#[test]
fn each_group_heading_appears_once_in_the_sidebar() {
    let mut headings: Vec<&str> = Vec::new();
    for item in PrefNav::ALL {
        if headings.last() != Some(&item.group()) {
            headings.push(item.group());
        }
    }
    let mut unique = headings.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        headings.len(),
        unique.len(),
        "repeated heading: {headings:?}"
    );
}

/// Mockup rows 143 and 144a: Language sits between Appearance and Keyboard shortcuts.
#[test]
fn the_sidebar_lists_language_between_appearance_and_keyboard_shortcuts() {
    let labels: Vec<&str> = PrefNav::ALL.iter().map(|item| item.label()).collect();
    assert_eq!(
        &labels[..5],
        [
            "Appearance",
            "Language",
            "Keyboard Shortcuts",
            "General",
            "Annotation"
        ]
    );
}

/// Mockup row 143: nav search also matches setting labels and shows their
/// page, so "wheel" finds Appearance (Mouse wheel) and no other page.
#[test]
fn a_search_for_a_setting_label_shows_its_page() {
    let en = signex_i18n::I18n::en_us();
    let found: Vec<PrefNav> = PrefNav::ALL
        .iter()
        .copied()
        .filter(|&item| super::graphite_fields::nav_matches(&en, item, "wheel"))
        .collect();
    assert_eq!(found, [PrefNav::Appearance]);
}

/// Mockup row 147: Autosave recovery copies and Allow agent connections
/// cover more than schematic editing, so a search for them shows System ›
/// General and not Editing.
#[test]
fn a_search_for_autosave_or_agents_shows_general() {
    let en = signex_i18n::I18n::en_us();
    for query in ["autosave", "agent connections"] {
        let found: Vec<PrefNav> = PrefNav::ALL
            .iter()
            .copied()
            .filter(|&item| super::graphite_fields::nav_matches(&en, item, query))
            .collect();
        assert_eq!(found, [PrefNav::General], "{query}");
    }
}

/// Mockup row 147: System › General sits in the System group, named
/// General in the nav and the subtitle.
#[test]
fn general_is_a_system_page() {
    let en = signex_i18n::I18n::en_us();
    assert_eq!(PrefNav::General.group(), "System");
    assert_eq!(
        signex_i18n::t!(en, super::graphite_fields::graphite_label(PrefNav::General)).into_string(),
        "General"
    );
}

/// Mockup row 143: the whole nav, page by page, with its group; PCB
/// Defaults (#2868, Caner 2026-11-24) after the Schematic pages.
#[test]
fn the_sidebar_follows_the_mock_groups() {
    let rows: Vec<(&str, &str)> = PrefNav::ALL
        .iter()
        .map(|item| (item.group(), item.label()))
        .collect();
    assert_eq!(
        rows,
        [
            ("System", "Appearance"),
            ("System", "Language"),
            ("System", "Keyboard Shortcuts"),
            ("System", "General"),
            ("Schematic", "Annotation"),
            ("Schematic", "Editing"),
            ("Schematic", "Display"),
            ("PCB", "PCB Defaults"),
            ("Validation", "Rule Profiles"),
            ("Library", "Distributor APIs"),
            ("Library", "Component Classes"),
        ]
    );
}
