//! The command catalog as the app sees it: the data lives in
//! `signex_commands::catalog`; rendering its message ids stays here.

use crate::keymap::{AppCommandId, ShortcutContext};
use signex_commands::catalog::{general, operands, pcb, schematic, threed};
use signex_i18n::{I18n, MessageId, command_ids, t};

pub use categories::category_title;
pub use signex_commands::catalog::{
    CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, HeadlessUse, Operand,
    OperandKind,
};

#[cfg(test)]
pub(crate) use operand_convention::{
    CONVENTION as OPERAND_CONVENTION, describe as describe_operands,
};

mod active_bar;
mod categories;
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

/// The message of the menu text of `command_id`, for row data that renders
/// later; `cmd-unnamed` for an id the catalog does not have.
pub fn command_menu_message(command_id: &str) -> MessageId {
    metadata_for_str(command_id).map_or(command_ids::CMD_UNNAMED, CommandMetadata::menu_label_id)
}

/// Every command-metadata table, one per surface group. `metadata_for`
/// and the tests iterate across all of them; the split keeps each table
/// file within the size cap without changing lookup behaviour (command
/// ids are unique across groups).
const TABLES: &[&[CommandMetadata]] = &[
    general::GENERAL,
    schematic::SCHEMATIC,
    active_bar::ACTIVE_BAR,
    pcb::PCB,
    threed::THREE_D,
];

/// Flattened iterator over every command's metadata, across all groups.
fn all_metadata() -> impl Iterator<Item = &'static CommandMetadata> {
    TABLES.iter().flat_map(|table| table.iter())
}

/// Every command id in the catalog, in table order.
///
/// Exists so a consumer can ask "what is in the catalog?" without
/// needing `CommandMetadata` itself — the bridge's coverage ratchet
/// (`app::command::bridge`) compares this against the ids
/// `core_to_message` actually matches.
pub fn all_command_ids() -> impl Iterator<Item = &'static str> {
    all_metadata().map(|metadata| metadata.id)
}

/// Id -> metadata index over every table, built once on first lookup.
///
/// The catalog is `const` data, so the index can never go stale. On a
/// duplicate id the first table entry wins, matching the linear `find`
/// it replaces. Menu rows, the shortcuts modal and the Active Bar
/// dropdown all resolve ids per row per frame (#1380).
fn metadata_index() -> &'static std::collections::HashMap<&'static str, &'static CommandMetadata> {
    static INDEX: std::sync::OnceLock<
        std::collections::HashMap<&'static str, &'static CommandMetadata>,
    > = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = std::collections::HashMap::new();
        for metadata in all_metadata() {
            index.entry(metadata.id).or_insert(metadata);
        }
        index
    })
}

pub fn metadata_for(command: &AppCommandId) -> Option<&'static CommandMetadata> {
    metadata_for_str(command.as_str())
}

/// [`metadata_for`] for a bare id string, so a per-row caller holding a
/// `&str` does not allocate an `AppCommandId` just to look it up.
pub fn metadata_for_str(id: &str) -> Option<&'static CommandMetadata> {
    metadata_index().get(id).copied()
}

/// The linear scan `metadata_for` used before the index; kept so a test
/// can prove the two agree for every catalog id.
#[cfg(test)]
fn metadata_for_linear(id: &str) -> Option<&'static CommandMetadata> {
    all_metadata().find(|metadata| metadata.id == id)
}

/// `General`-group commands that act without an editor: file and tab
/// management, preferences, panels, search, the palette, the shortcuts
/// pane, history (`is_enabled` gates it on the real stacks) and the
/// project-wide jobs that assemble every sheet (`is_enabled` gates those on
/// `has_sheets`, the predicate that enables their menu rows, so palette and
/// menu agree on a PCB tab or a project with no tab open).
///
/// Every other `General` command is editor-scoped. The list is the
/// exception on purpose: a new `General` command stays off the start
/// screen until someone decides it is global.
const GLOBAL_COMMANDS: &[&str] = &[
    "new_document",
    "new_project_with_profile",
    "open_document",
    "save_document",
    "save_document_as",
    "print",
    "close_active_document",
    "close_all_documents",
    "tile_windows_horizontally",
    "tile_windows_vertically",
    "next_document_tab",
    "previous_document_tab",
    "undo",
    "redo",
    "open_command_palette",
    "open_preferences",
    "open_schematic_preferences",
    "open_library_browser",
    "open_net_inspector",
    "show_current_command_hotkeys",
    "show_current_command_shortcuts",
    "toggle_floating_panels",
    "toggle_properties_panel",
    "toggle_schematic_filter_panel",
    "toggle_schematic_list_panel",
    "toggle_search_panel",
    "toggle_auto_focus",
    "find",
    "find_and_replace",
    "find_next",
    "find_previous",
    "find_text",
    "report_manager_bom",
    "annotate_schematic_quietly",
    "force_annotate_all_schematics",
    "run_erc",
    "open_erc_dialog",
    "number_schematic_sheets",
    "update_pcb_from_schematic",
    "back_annotate_schematics",
    "new_pcb_document",
    "new_schematic_sheet",
];

/// The editor contexts in which `command` can act, or `None` when it is
/// global (File, Preferences, panels, the palette itself).
///
/// Derived from the command's own metadata, never from a keymap profile's
/// binding sections: the built-in profiles bind nearly everything under
/// `global`, so a binding says nothing about where a command belongs.
///
/// 1. Ids in [`GLOBAL_COMMANDS`] are global.
/// 2. A few `General` commands act in one editor (`break_wire`, the grid
///    picker); they are listed here.
/// 3. `Enablement::RequiresDocument(kind)` names the editor outright.
/// 4. The `Schematic` / `Pcb` / `ThreeD` surface group decides.
/// 5. Any other `General` command is editor-scoped.
pub fn required_contexts(command: &AppCommandId) -> Option<&'static [ShortcutContext]> {
    const EDITORS: &[ShortcutContext] = &[
        ShortcutContext::Schematic,
        ShortcutContext::Pcb,
        ShortcutContext::Footprint,
        ShortcutContext::Library,
    ];
    if GLOBAL_COMMANDS.contains(&command.as_str()) {
        return None;
    }
    match command.as_str() {
        "break_wire"
        | "drag_keep_connections"
        | "cycle_wire_bus_graphic_mode"
        | "cycle_wiring_mode"
        | "switch_segment_posture" => return Some(&[ShortcutContext::Schematic]),
        "open_grid_picker" | "cycle_snap_grid_forward" => {
            return Some(&[ShortcutContext::Footprint]);
        }
        _ => {}
    }
    // An id with no metadata is not in the catalog, so nothing to offer.
    let metadata = metadata_for(command)?;
    if let Enablement::RequiresDocument(kind) = metadata.enable {
        return Some(match kind {
            DocumentKind::Schematic => &[ShortcutContext::Schematic],
            DocumentKind::Pcb => &[ShortcutContext::Pcb],
            DocumentKind::Footprint => &[ShortcutContext::Footprint],
            DocumentKind::Symbol | DocumentKind::Library => &[ShortcutContext::Library],
        });
    }
    match metadata.group {
        CommandGroup::Schematic => Some(&[ShortcutContext::Schematic]),
        CommandGroup::Pcb => Some(&[ShortcutContext::Pcb]),
        CommandGroup::ThreeD => Some(&[ShortcutContext::Footprint]),
        CommandGroup::General => Some(EDITORS),
    }
}

/// Whether `command` may be offered while `live` contexts are active.
pub fn offered_in(command: &AppCommandId, live: &[ShortcutContext]) -> bool {
    required_contexts(command).is_none_or(|needed| needed.iter().any(|c| live.contains(c)))
}

pub fn fallback_label(command: &AppCommandId) -> String {
    categories::title_words(command.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_of(id: &str) -> CommandGroup {
        all_metadata()
            .find(|metadata| metadata.id == id)
            .unwrap_or_else(|| panic!("missing command metadata for `{id}`"))
            .group
    }

    #[test]
    fn indexed_metadata_lookup_equals_the_linear_scan_for_every_id() {
        let mut count = 0;
        for metadata in all_metadata() {
            let command = AppCommandId::new(metadata.id).expect("catalog ids are valid");
            let indexed = metadata_for(&command).expect("indexed lookup finds every catalog id");
            let linear = metadata_for_linear(metadata.id).expect("linear finds it");
            assert!(
                std::ptr::eq(indexed, linear),
                "`{}` resolved differently",
                metadata.id
            );
            count += 1;
        }
        assert!(count > 100, "catalog unexpectedly small: {count}");
        let unknown = AppCommandId::new("definitely_not_a_command").expect("valid id shape");
        assert!(metadata_for(&unknown).is_none());
    }

    #[test]
    fn every_command_has_a_group_in_display_order() {
        // Catches an entry that was left with a stray / unlisted group.
        for metadata in all_metadata() {
            assert!(
                CommandGroup::ALL.contains(&metadata.group),
                "command `{}` has a group absent from CommandGroup::ALL",
                metadata.id
            );
        }
    }

    #[test]
    fn command_groups_follow_primary_surface() {
        // Schematic: placement, design, ERC, PCB-sync, net, sheet-nav.
        assert_eq!(group_of("place_wire"), CommandGroup::Schematic);
        assert_eq!(group_of("place_symbol"), CommandGroup::Schematic);
        assert_eq!(group_of("run_erc"), CommandGroup::Schematic);
        assert_eq!(group_of("annotate_schematic"), CommandGroup::Schematic);
        assert_eq!(
            group_of("update_pcb_from_schematic"),
            CommandGroup::Schematic
        );
        assert_eq!(group_of("next_sheet"), CommandGroup::Schematic);
        assert_eq!(
            group_of("highlight_net_under_cursor"),
            CommandGroup::Schematic
        );
        assert_eq!(group_of("open_components_panel"), CommandGroup::Schematic);
        // Footprint editor / 3D view.
        assert_eq!(group_of("footprint_mode_pads"), CommandGroup::ThreeD);
        assert_eq!(group_of("footprint_mode_view_3d"), CommandGroup::ThreeD);
        // Shared editing / transform / view / file → General.
        assert_eq!(group_of("copy"), CommandGroup::General);
        assert_eq!(group_of("rotate_clockwise"), CommandGroup::General);
        assert_eq!(group_of("mirror_x"), CommandGroup::General);
        assert_eq!(group_of("zoom_to_fit"), CommandGroup::General);
        assert_eq!(group_of("open_preferences"), CommandGroup::General);
    }

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

    #[test]
    fn grouping_partitions_every_command_exactly_once() {
        let summed: usize = CommandGroup::ALL
            .iter()
            .map(|group| {
                all_metadata()
                    .filter(|metadata| metadata.group == *group)
                    .count()
            })
            .sum();
        assert_eq!(
            summed,
            all_metadata().count(),
            "each command must land in exactly one CommandGroup"
        );
        // The two primary EDA surfaces must carry commands.
        assert!(all_metadata().any(|metadata| metadata.group == CommandGroup::Schematic));
        assert!(all_metadata().any(|metadata| metadata.group == CommandGroup::ThreeD));
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
                categories::category_title_id(metadata.category).is_some(),
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

    #[test]
    fn icon_and_keybind_still_inherit_the_default() {
        // `icon` and `keybind` are the two descriptor fields still
        // unpopulated: an icon needs a design decision per command, and a
        // catalog `keybind` is only a *suggested* default that any profile
        // binding overrides, so nothing reads it yet. Asserting them for
        // every entry catches a row that drops its
        // `..CommandMetadata::DEFAULT` tail and silently changes what it
        // inherits.
        //
        // `enable` and `flags` used to be asserted here too, at their
        // defaults. They are now populated per command
        // (`every_catalog_entry_sets_enable_and_flags` below), so pinning
        // them at the default would pin the *absence* of the decision.
        for metadata in all_metadata() {
            assert_eq!(
                metadata.icon, None,
                "`{}` should default to no icon",
                metadata.id
            );
            assert_eq!(
                metadata.keybind, None,
                "`{}` should default to no keybind",
                metadata.id
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

    /// Every catalog row must state `enable` and `flags` explicitly.
    ///
    /// A source scan rather than a value check, because the two cannot be
    /// told apart at runtime: `Enablement::Always` + `CommandFlags::NONE`
    /// is both a legitimate answer and what an unpopulated row inherits
    /// from `..CommandMetadata::DEFAULT`. Only the source says whether
    /// somebody decided. Mirrors the source-scanning style of
    /// `keymap::menu_command_tests` and `app::command::bridge`'s guards.
    #[test]
    fn every_catalog_entry_sets_enable_and_flags() {
        const TABLE_SRC: &[(&str, &str)] = &[
            (
                "general.rs",
                include_str!("../../../../signex-commands/src/catalog/general.rs"),
            ),
            (
                "schematic.rs",
                include_str!("../../../../signex-commands/src/catalog/schematic.rs"),
            ),
            ("active_bar.rs", include_str!("active_bar.rs")),
            (
                "pcb.rs",
                include_str!("../../../../signex-commands/src/catalog/pcb.rs"),
            ),
            (
                "threed.rs",
                include_str!("../../../../signex-commands/src/catalog/threed.rs"),
            ),
        ];

        let mut missing: Vec<String> = Vec::new();
        let mut seen = 0usize;
        for (file, src) in TABLE_SRC {
            for entry in src.split("    CommandMetadata {").skip(1) {
                let body = entry.split("    },").next().unwrap_or(entry);
                let id = body
                    .split("id: \"")
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .unwrap_or("<unparsed>");
                seen += 1;
                if !body.contains("enable: ") {
                    missing.push(format!("{file}: `{id}` has no `enable:`"));
                }
                if !body.contains("flags: ") {
                    missing.push(format!("{file}: `{id}` has no `flags:`"));
                }
            }
        }

        assert_eq!(
            seen,
            all_metadata().count(),
            "the source scan found {seen} entries but the tables hold {} — \
             the `CommandMetadata {{` split in this test has drifted from \
             how the tables are written",
            all_metadata().count()
        );
        assert!(
            missing.is_empty(),
            "catalog entries must state `enable` and `flags` rather than \
             inheriting them from ..CommandMetadata::DEFAULT — an inherited \
             `Always` / all-false is indistinguishable from nobody having \
             decided: {missing:#?}"
        );
    }
}
