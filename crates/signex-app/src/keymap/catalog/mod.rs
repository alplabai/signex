//! The command catalog as the app sees it: the data lives in
//! `signex_commands::catalog`; rendering its message ids stays here.

#[cfg(test)]
use crate::keymap::AppCommandId;
#[cfg(test)]
use signex_commands::catalog::{CommandFlags, all_metadata};
use signex_i18n::{I18n, t};

pub use signex_commands::catalog::{
    CommandGroup, CommandMetadata, HeadlessUse, Operand, OperandKind, all_command_ids,
    command_menu_message, fallback_label, metadata_for, metadata_for_str, offered_in,
};

#[cfg(test)]
pub(crate) use operand_convention::{
    CONVENTION as OPERAND_CONVENTION, describe as describe_operands,
};

#[cfg(test)]
mod operand_convention;

/// The text of a catalog row in a message language. The catalog itself is
/// UI-free (`signex-commands`); every caller that renders a row imports this.
pub trait CatalogTextExt {
    /// Why a headless caller is refused this command: `Some` for a command
    /// that only works in a window, with the one wording every headless
    /// surface reports (#2170).
    fn headless_refusal(&self) -> Option<String>;

    /// The descriptive label in the message language.
    fn label_in(&self, i18n: &I18n) -> String;

    /// The descriptive label in en-US, for the CLI catalog and other
    /// surfaces that are English by contract.
    fn label_en_us(&self) -> String;

    /// The label a menu surface should display: the terse `menu_label`
    /// override when present, else the descriptive `label`.
    fn menu_label_in(&self, i18n: &I18n) -> String;
}

impl CatalogTextExt for CommandMetadata {
    fn headless_refusal(&self) -> Option<String> {
        (self.headless_use() == Some(HeadlessUse::WindowOnly))
            .then(|| format!("`{}` only works in a window", self.id))
    }

    fn label_in(&self, i18n: &I18n) -> String {
        t!(i18n, self.label).to_string()
    }

    fn label_en_us(&self) -> String {
        self.label_in(&I18n::en_us())
    }

    fn menu_label_in(&self, i18n: &I18n) -> String {
        t!(i18n, self.menu_label_id()).to_string()
    }
}

/// The menu text of the command `command_id` in the message language; the
/// bare id for one the catalog does not have.
pub fn command_menu_text(i18n: &I18n, command_id: &str) -> String {
    metadata_for_str(command_id).map_or_else(
        || command_id.to_string(),
        |metadata| metadata.menu_label_in(i18n),
    )
}

/// The title of `category` in the message language; an id the catalog does
/// not know is spelled out from its words (`idle_tools` -> `Idle Tools`).
pub fn category_title(i18n: &I18n, category: &str) -> String {
    match signex_commands::catalog::category_title_id(category) {
        Some(id) => t!(i18n, id).to_string(),
        None => signex_commands::catalog::title_words(category),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #2170: a new row with no headless decision fails here. Decide it: set
    /// `GUI_ONLY` (window UI only), or give it an effect (a mutating flag,
    /// operands, or `RUNS_HEADLESS`).
    #[test]
    fn every_command_is_classified_for_headless_use() {
        let mut window_only = 0;
        for metadata in all_metadata() {
            let flags = metadata.flags;
            assert!(
                !(flags.gui_only && flags.runs_headless),
                "`{}` is both gui_only and runs_headless",
                metadata.id
            );
            match metadata.headless_use() {
                Some(HeadlessUse::WindowOnly) => window_only += 1,
                Some(HeadlessUse::Runs) => {}
                None => panic!("`{}` has no headless classification", metadata.id),
            }
            assert_eq!(
                metadata.headless_refusal().is_some(),
                flags.gui_only,
                "`{}`: the refusal follows gui_only",
                metadata.id
            );
        }
        assert!(window_only > 100, "window-only rows: {window_only}");
    }

    #[test]
    fn the_headless_refusal_names_the_command_and_the_window() {
        let id = |name: &str| metadata_for_str(name).expect("catalog row");
        assert_eq!(
            id("open_erc_dialog").headless_refusal().as_deref(),
            Some("`open_erc_dialog` only works in a window")
        );
        assert_eq!(id("run_erc").headless_use(), Some(HeadlessUse::Runs));
        assert!(id("run_erc").headless_refusal().is_none());
        assert!(id("rotate_selection").headless_refusal().is_none());
    }

    /// A row with no flag and no operands is undecided, never silently runnable.
    #[test]
    fn an_undecided_row_has_no_headless_use() {
        let undecided = CommandMetadata {
            id: "undecided",
            flags: CommandFlags::NONE,
            ..CommandMetadata::DEFAULT
        };
        assert_eq!(undecided.headless_use(), None);
        assert_eq!(undecided.headless_refusal(), None);
        let both = CommandMetadata {
            flags: CommandFlags {
                gui_only: true,
                runs_headless: true,
                ..CommandFlags::NONE
            },
            ..undecided
        };
        assert_eq!(both.headless_use(), None);
    }

    /// #2057: labels and category titles live in the en-US catalog; every
    /// command renders a real one, and a few keep their pre-migration English.
    #[test]
    fn every_command_label_and_category_title_comes_from_the_catalog() {
        let en = I18n::en_us_ref();
        for metadata in all_metadata() {
            let label = metadata.label_in(en);
            assert!(!label.is_empty(), "`{}` has an empty label", metadata.id);
            assert_ne!(
                label,
                metadata.label.as_str(),
                "`{}` label missing",
                metadata.id
            );
            let menu = metadata.menu_label_in(en);
            assert!(
                !menu.is_empty(),
                "`{}` has an empty menu label",
                metadata.id
            );
            assert!(
                signex_commands::catalog::category_title_id(metadata.category).is_some(),
                "category `{}` has no title",
                metadata.category
            );
        }
        let text = |id: &str| {
            let md = metadata_for(&AppCommandId::new(id).unwrap()).unwrap();
            (md.label_in(en), md.menu_label_in(en))
        };
        assert_eq!(
            text("run_drc"),
            (
                "Run the design rule check on the board".to_string(),
                "Design Rule Check".to_string()
            )
        );
        assert_eq!(
            text("place_wire"),
            ("Place wire".to_string(), "Wire".to_string())
        );
        assert_eq!(category_title(en, "pcb_sync"), "Pcb Sync");
        assert_eq!(category_title(en, "navigation"), "Navigation");
        assert_eq!(category_title(en, "uncategorized"), "Uncategorized");
    }

    #[test]
    fn menu_label_falls_back_to_label_when_unset() {
        // No override → menus show the descriptive label unchanged.
        let copy = metadata_for(&AppCommandId::new("copy").unwrap()).unwrap();
        assert_eq!(copy.menu_label, None);
        assert_eq!(
            copy.menu_label_in(I18n::en_us_ref()),
            copy.label_in(I18n::en_us_ref())
        );
        assert_eq!(copy.menu_label_in(I18n::en_us_ref()), "Copy");
        // Override present → menus get the terse form while `label` keeps
        // the descriptive phrasing for the Keyboard Shortcuts pane.
        let wire = metadata_for(&AppCommandId::new("place_wire").unwrap()).unwrap();
        assert_eq!(wire.label_in(I18n::en_us_ref()), "Place wire");
        assert_eq!(wire.menu_label_in(I18n::en_us_ref()), "Wire");
    }

    #[test]
    fn menu_label_overrides_match_menu_bar_text() {
        // Locks the terse menu labels to what menu_bar renders today, so a
        // catalog edit that would silently change a menu's visible text
        // fails here (drift guard toward #271/#272). (command id, menu text)
        const MENU: &[(&str, &str)] = &[
            ("new_document", "New Project"),
            ("open_document", "Open..."),
            ("save_document", "Save"),
            ("save_document_as", "Save As..."),
            ("smart_paste", "Paste Special"),
            ("delete_selection", "Delete"),
            ("select_all", "Select All"),
            ("zoom_to_fit", "Fit All"),
            ("zoom_in_at_cursor", "Zoom In"),
            ("toggle_visible_grid", "Toggle Grid"),
            ("place_wire", "Wire"),
            ("place_bus", "Bus"),
            ("place_net_label", "Net Label"),
            ("open_components_panel", "Component..."),
            ("run_erc", "Electrical Rules Check"),
            ("open_erc_dialog", "Electrical Rules Check"),
            ("next_grid", "Cycle Grid Size"),
            ("open_preferences", "Preferences..."),
            ("show_current_command_hotkeys", "Keyboard Shortcuts"),
        ];
        for (id, expected) in MENU {
            let md = metadata_for(&AppCommandId::new(*id).unwrap())
                .unwrap_or_else(|| panic!("missing catalog entry for `{id}`"));
            assert_eq!(
                md.menu_label_in(I18n::en_us_ref()),
                *expected,
                "menu label drift for `{id}`"
            );
        }
    }

    /// Golden-snapshot test (signex#276): locks the STABLE command-id
    /// surface — `id` + `group` + `category` — because external CLI/plugin
    /// tooling depends on ids staying put. The churny descriptor fields
    /// (`icon`/`keybind`/`enable`/`flags`, added by #275/#479) are
    /// deliberately NOT captured here and may change freely.
    ///
    /// A failing diff means one of two things:
    ///   - a command was renamed or removed. This is a breaking change for
    ///     downstream consumers: add an alias + a deprecation note, do NOT
    ///     just regenerate the golden to make the test pass again.
    ///   - a command was added, or an existing one's `group`/`category`
    ///     changed on purpose. Regenerate with:
    ///     `UPDATE_GOLDEN=1 cargo test -p signex-app command_id_surface_matches_golden_snapshot`
    #[test]
    fn command_id_surface_matches_golden_snapshot() {
        #[derive(serde::Serialize)]
        struct CommandIdSnapshot {
            id: &'static str,
            group: String,
            category: &'static str,
        }

        let mut snapshot: Vec<CommandIdSnapshot> = all_metadata()
            .map(|metadata| CommandIdSnapshot {
                id: metadata.id,
                group: format!("{:?}", metadata.group),
                category: metadata.category,
            })
            .collect();
        snapshot.sort_by_key(|entry| entry.id);

        let actual = serde_json::to_string_pretty(&snapshot).unwrap() + "\n";
        let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/commands.json");

        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(golden_path, &actual)
                .unwrap_or_else(|err| panic!("failed to write golden {golden_path}: {err}"));
            return;
        }

        let expected = std::fs::read_to_string(golden_path)
            .unwrap_or_else(|err| panic!("failed to read golden {golden_path}: {err}"));
        assert_eq!(
            actual, expected,
            "command-id surface drifted from crates/signex-app/tests/golden/commands.json — \
             see this test's doc comment before regenerating"
        );
    }
}
