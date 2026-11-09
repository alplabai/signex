//! Test helpers that read a built [`iced::menu::Menu`] by the same
//! `Menu/Sub/Row` paths the model tests use.

use iced::menu::{Action, Item, Menu};

use super::SHORTCUT_GAP;
use crate::app::Message;

/// A row label matches a path segment when it is that text, with or without
/// the keymap shortcut appended.
fn label_is(label: &str, segment: &str) -> bool {
    label == segment
        || label
            .strip_prefix(segment)
            .is_some_and(|rest| rest.starts_with(SHORTCUT_GAP))
}

fn walk<'a>(items: &'a [Item<Message>], path: &[&str]) -> Option<&'a Action<Message>> {
    let (first, rest) = path.split_first()?;
    items.iter().find_map(|item| match item {
        Item::Action(action) if rest.is_empty() && label_is(&action.label, first) => Some(action),
        Item::Submenu(sub) if !rest.is_empty() && sub.label == *first => walk(&sub.items, rest),
        _ => None,
    })
}

/// The row at `path` (for example `File/Export/PDF…`), live or greyed.
pub(crate) fn action_at<'a>(menu: &'a Menu<Message>, path: &str) -> Option<&'a Action<Message>> {
    walk(menu.items(), &path.split('/').collect::<Vec<_>>())
}

/// The message the row at `path` sends, `None` when it is greyed or absent.
pub(crate) fn row_message(menu: &Menu<Message>, path: &str) -> Option<Message> {
    action_at(menu, path).and_then(|action| action.on_activate.clone())
}

/// The titles of the top-level menus, the app menu first.
pub(crate) fn root_titles(menu: &Menu<Message>) -> Vec<String> {
    menu.items()
        .iter()
        .filter_map(|item| match item {
            Item::Submenu(sub) => Some(sub.label.clone()),
            _ => None,
        })
        .collect()
}
