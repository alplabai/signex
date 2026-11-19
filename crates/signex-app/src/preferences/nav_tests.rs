//! #1769 - the sidebar shows each group heading once.

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

/// #143 / #144a: Language sits between Appearance and Keyboard shortcuts.
#[test]
fn the_sidebar_lists_language_between_appearance_and_keyboard_shortcuts() {
    let labels: Vec<&str> = PrefNav::ALL.iter().map(|item| item.label()).collect();
    assert_eq!(
        &labels[..4],
        ["Appearance", "Language", "Keyboard Shortcuts", "Annotation"]
    );
}

/// #143: the whole nav, page by page, with its group.
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
            ("Schematic", "Annotation"),
            ("Schematic", "Editing"),
            ("Validation", "Rule Profiles"),
            ("Library", "Distributor APIs"),
            ("Library", "Component Classes"),
        ]
    );
}
