//! Help ▸ Keyboard Shortcuts: a reference of the active profile's working
//! shortcuts, grouped by category in two columns, with a filter (action,
//! key or command id) and a Schematic / PCB / All scope. Opened from the
//! Help menu and by F1, which also closes it.
//!
//! `sections` builds the rows (cached in `UiState`), `filter` decides what
//! a filter and a scope show (the scope rule is on [`ShortcutScope`]) and
//! `view` lays the result out.

mod filter;
mod sections;
#[cfg(test)]
mod sheet_tests;
mod view;

use signex_i18n::I18n;

use crate::keymap::CompiledKeymap;

pub use filter::{ShortcutScope, ShortcutSheet};
pub use sections::{ShortcutSection, build_sections, build_sections_with};
pub(crate) use view::key_cap;
pub use view::{SheetFacts, view, window_view};

/// The key that opens and closes the sheet, as its footer and the
/// first-run card name it: the profile's binding of the sheet command,
/// preferring F1, else F1 itself, which `app/dispatch/input.rs` claims in
/// every profile.
pub fn sheet_key(keymap: &CompiledKeymap, i18n: &I18n) -> String {
    let bound = keymap.shortcut_label_preferring("show_current_command_hotkeys", "F1", i18n);
    if bound.is_empty() {
        crate::keymap::shown_in("F1", i18n)
    } else {
        bound
    }
}
