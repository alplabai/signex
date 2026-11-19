//! The sheet's rows: the active profile's working shortcuts grouped by
//! category, built once per keymap change and cached in `UiState`.

use signex_i18n::I18n;

use crate::keymap::{KeymapEditorModel, KeymapEditorRow, ShortcutProfileSet};

/// One category of the reference sheet: the category id and the rows
/// listed under it, in profile order. A row is the keymap editor's row, so
/// the sheet's filter matches exactly what the Preferences keymap search
/// matches (label, key, command id) and its scope reads the row's context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutSection {
    pub category: String,
    pub rows: Vec<KeymapEditorRow>,
}

/// Group the active profile's listed shortcuts by category, categories in
/// first-seen order. Pure: the result depends only on `profiles`, so
/// `UiState` caches it and rebuilds on a keymap change instead of the view
/// rebuilding it every frame (#1380).
pub fn build_sections(i18n: &I18n, profiles: &ShortcutProfileSet) -> Vec<ShortcutSection> {
    let editor = KeymapEditorModel::new(profiles.clone());
    let mut sections: Vec<ShortcutSection> = Vec::new();
    let mut index = std::collections::HashMap::<String, usize>::new();
    for row in editor.rows_in(i18n) {
        let slot = *index.entry(row.category.clone()).or_insert_with(|| {
            sections.push(ShortcutSection {
                category: row.category.clone(),
                rows: Vec::new(),
            });
            sections.len() - 1
        });
        if !row.trigger.trim().is_empty() && is_listed(&row) {
            sections[slot].rows.push(row);
        }
    }
    sections.retain(|section| !section.rows.is_empty());
    sections
}

/// Whether the reference sheet may list `row` as a working shortcut.
///
/// A catalog command with no dispatch arm consumes its key and does
/// nothing (#103), so listing it would advertise a dead binding. Rows
/// without a command id are kept as they were.
fn is_listed(row: &KeymapEditorRow) -> bool {
    !row.unbound
        && row
            .command
            .as_ref()
            .is_none_or(crate::app::command::bridge::is_dispatchable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::category_title;

    #[test]
    fn category_titles_capitalise_by_the_message_language() {
        let tr = I18n::with_message_locale_for_test("tr");
        // A title comes from the catalog (`tr` falls back to en-US until
        // #2065 translates it), so it no longer takes Turkish casing.
        assert_eq!(category_title(&tr, "idle_tools"), "Idle Tools");
        assert_eq!(category_title(&I18n::en_us(), "idle_tools"), "Idle Tools");
        assert_eq!(tr.upper_for_display("ilk"), "İLK");
    }

    /// The pre-cache algorithm (#1380), kept as the oracle.
    fn legacy_sections(profiles: &ShortcutProfileSet) -> Vec<(String, Vec<(String, String)>)> {
        let rows = KeymapEditorModel::new(profiles.clone()).rows();
        let mut categories = Vec::<String>::new();
        for row in &rows {
            if !categories.iter().any(|category| category == &row.category) {
                categories.push(row.category.clone());
            }
        }
        categories
            .into_iter()
            .map(|category| {
                let listed = rows
                    .iter()
                    .filter(|row| {
                        row.category == category && !row.trigger.trim().is_empty() && is_listed(row)
                    })
                    .map(|row| (row.label.clone(), row.trigger.clone()))
                    .collect::<Vec<_>>();
                (category, listed)
            })
            .filter(|(_, listed)| !listed.is_empty())
            .collect()
    }

    #[test]
    fn build_sections_equals_the_per_frame_algorithm_for_every_profile() {
        let mut set = ShortcutProfileSet::built_ins().expect("built-ins load");
        let ids: Vec<String> = set.profiles().map(|p| p.id.clone()).collect();
        for id in ids {
            set.set_active_profile(id).unwrap();
            let built: Vec<_> = build_sections(I18n::en_us_ref(), &set)
                .into_iter()
                .map(|s| {
                    let rows = s
                        .rows
                        .into_iter()
                        .map(|row| (row.label, row.trigger))
                        .collect::<Vec<_>>();
                    (s.category, rows)
                })
                .collect();
            assert_eq!(built, legacy_sections(&set));
            assert!(!built.is_empty());
        }
    }

    /// #103 - the sheet never lists a binding whose command has no
    /// dispatch arm, and still lists the ones that work.
    #[test]
    fn the_sheet_lists_only_shortcuts_that_dispatch() {
        /// The rows with a trigger, split into listed and hidden.
        fn split(rows: &[KeymapEditorRow]) -> (Vec<&KeymapEditorRow>, Vec<&KeymapEditorRow>) {
            rows.iter()
                .filter(|row| !row.trigger.trim().is_empty())
                .partition(|r| is_listed(r))
        }
        let mut model = KeymapEditorModel::built_ins().expect("built-in profiles load");
        let is_bound_to = |rows: &[&KeymapEditorRow], id: &str| {
            rows.iter()
                .any(|r| r.command.as_ref().is_some_and(|c| c.as_str() == id))
        };
        let all_dispatch = |listed: &[&KeymapEditorRow]| {
            listed.iter().all(|r| {
                r.command
                    .as_ref()
                    .is_none_or(crate::app::command::bridge::is_dispatchable)
            })
        };

        // The default profile.
        let rows = model.rows();
        let (listed, _) = split(&rows);
        assert!(
            listed
                .iter()
                .any(|r| r.label.to_lowercase().contains("undo")),
            "a working shortcut must stay listed"
        );
        assert!(
            is_bound_to(&listed, "cycle_wiring_mode"),
            "cycle_wiring_mode dispatches (#1669), so its binding is listed"
        );
        assert!(all_dispatch(&listed));

        // Classic binds the still unmapped `reset_local_coordinates`.
        model.set_active_profile("classic").expect("classic loads");
        let rows = model.rows();
        let (listed, hidden) = split(&rows);
        assert!(
            is_bound_to(&hidden, "reset_local_coordinates"),
            "an unimplemented command's binding must be hidden"
        );
        assert!(all_dispatch(&listed));
    }
}
