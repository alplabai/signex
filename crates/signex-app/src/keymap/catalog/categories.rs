//! Category titles. A command's `category` is a stable id; its title is a
//! catalog message (`cmd-category-<id>`), so the Keyboard Shortcuts pane and
//! the preferences keymap list can translate it.

use signex_i18n::{I18n, MessageId, command_ids as ids, t};

/// The title message of a catalog category, or `None` for an id that is not
/// one (the editor's `uncategorized` bucket aside, which is).
pub(super) fn category_title_id(category: &str) -> Option<MessageId> {
    Some(match category {
        "edit" => ids::CMD_CATEGORY_EDIT,
        "modify" => ids::CMD_CATEGORY_MODIFY,
        "interactive" => ids::CMD_CATEGORY_INTERACTIVE,
        "view" => ids::CMD_CATEGORY_VIEW,
        "validation" => ids::CMD_CATEGORY_VALIDATION,
        "file" => ids::CMD_CATEGORY_FILE,
        "window" => ids::CMD_CATEGORY_WINDOW,
        "select" => ids::CMD_CATEGORY_SELECT,
        "library" => ids::CMD_CATEGORY_LIBRARY,
        "search" => ids::CMD_CATEGORY_SEARCH,
        "commands" => ids::CMD_CATEGORY_COMMANDS,
        "panels" => ids::CMD_CATEGORY_PANELS,
        "preferences" => ids::CMD_CATEGORY_PREFERENCES,
        "reports" => ids::CMD_CATEGORY_REPORTS,
        "help" => ids::CMD_CATEGORY_HELP,
        "design" => ids::CMD_CATEGORY_DESIGN,
        "place" => ids::CMD_CATEGORY_PLACE,
        "navigation" => ids::CMD_CATEGORY_NAVIGATION,
        "pcb_sync" => ids::CMD_CATEGORY_PCB_SYNC,
        "pcb" => ids::CMD_CATEGORY_PCB,
        "uncategorized" => ids::CMD_CATEGORY_UNCATEGORIZED,
        _ => return None,
    })
}

/// The title of `category` in the message language; an id the catalog does
/// not know is spelled out from its words (`idle_tools` -> `Idle Tools`).
pub fn category_title(i18n: &I18n, category: &str) -> String {
    match category_title_id(category) {
        Some(id) => t!(i18n, id).to_string(),
        None => title_words(category),
    }
}

/// `a_b` -> `A B`: each word's first ASCII letter capitalised. The text of a
/// command or category the catalog has no message for.
pub(super) fn title_words(value: &str) -> String {
    value
        .split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_ascii_uppercase().to_string();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
