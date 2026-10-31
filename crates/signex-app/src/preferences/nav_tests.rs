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

#[test]
fn the_sidebar_lists_keyboard_shortcuts_right_after_appearance() {
    let labels: Vec<&str> = PrefNav::ALL.iter().map(|item| item.label()).collect();
    assert_eq!(
        &labels[..4],
        ["Appearance", "Keyboard Shortcuts", "Annotation", "Editing"]
    );
}
